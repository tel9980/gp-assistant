# CI 构建与发布

两个 workflow，都在 `.github/workflows/`：

| 文件 | 触发 | 产物 |
| --- | --- | --- |
| `frontend.yml` | push / PR | 前端 735 用例 + 7 个契约脚本 + gp-core 的 fmt/test/clippy（不发布） |
| `release-windows.yml` | 手动 `workflow_dispatch` | Windows 安装包 + 免安装版 → GitHub Release |

## 发 Windows 安装包 / 免安装版

Actions → **Release Windows installer** → Run workflow。可填 `tag`，留空则自动用
`v<version>-ci.<run_number>`，并标为预发布。产物名统一为
`guxuanyou_<version>_windows_x64_setup.exe`（NSIS 默认名会把中文 productName 清成空，workflow 里已改名）。

一次运行会产出三个资产：

| 资产 | 形态 | 说明 |
| --- | --- | --- |
| `guxuanyou_<ver>_windows_x64_setup.exe` | 安装版（NSIS） | 装到 Program Files，有开始菜单项 |
| `guxuanyou_<ver>_windows_x64_portable.zip` | **免安装版** | 解压即用，内含 `guxuanyou.exe` + `models/`（本地向量模型） |
| `guxuanyou_<ver>_windows_x64_portable.exe` | 免安装单文件 | 仅主程序；需要本地向量检索时把 `models/` 放到同目录 |

免安装版为什么必须把 `models/` 放在一起：Tauri 对未安装的 exe 会把 `resource_dir()`
解析成 **exe 所在目录**，而 `src-tauri/src/research.rs` 的 `research_embedding_model_dir`
就在 `resource_dir()/models/bge-small-zh-v1.5-int8` 找模型；找不到会退化到开发路径
（`CARGO_MANIFEST_DIR/../../models/...`），在用户机器上不存在，于是本地向量检索不可用。

注意事项：

- 三个产物都**没有** Authenticode 签名，首次运行 Windows SmartScreen 会提示「未知发布者」。
- 免安装版依赖系统自带 WebView2 运行时（Win10 1803+ / Win11 默认已安装）。
- 构建走 `desktop/package.json` 的 `build:windows`，会先跑 `prepare:windows`
  （下载 RAG 嵌入模型 + 构建前端 + 准备 Android 资源），再 `tauri build --features gepa-lab`。
- 首次冷构建约 25 分钟（Rust 依赖全量编译），rust-cache 命中后约 10 分钟。

## 已知限制

- `scripts/release-check.ps1` 依赖 PowerShell 与 Windows 打包链路，CI 里不跑；
  本机要发正式版仍用它。
- `.github/workflows/rag-evaluation.yml` 需要外部模型服务，CI 上的失败多为环境缺失。
- 曾经加过「评测/开发镜像」（GHCR）的 workflow 与 Dockerfile，已移除：本项目是 Tauri 桌面 GUI，
  容器里只能装工具链、装不出能跑的应用，留着是死重量。
## 覆盖率门禁

`vitest.config.ts` 的 coverage 统计 **src/lib + src/hooks + src/components**（2026-11-05 起，
此前只有 src/lib）。阈值两条：

- 全局：statements 74 / branches 65 / functions 71 / lines 78
- `src/lib/**`（更严）：74 / 62 / 72 / 78

vitest 支持按 glob 设 threshold，已验证会真正执行（把某条调到 99% 会报
`ERROR: Coverage for statements ... does not meet "src/lib/**/*.{ts,tsx}" threshold`）。

## Android APK（`.github/workflows/release-android.yml`）

只支持手动触发（`workflow_dispatch`），不属于日常门禁：首次构建 20+ 分钟，之后靠
rust-cache 约 10 分钟。产物上传为构建工件 `guxuanyou-android-apk`（保留 14 天）。

宿主刻意选 **windows-latest**：`scripts/prepare-tauri-android-assets.ps1` 里固定调用
`npm.cmd` 并写 Windows 风格路径，换到 ubuntu 需要重写 961 行脚本，风险大于收益。

流程：`actions/setup-java`（temurin 17）→ `android-actions/setup-android` →
`sdkmanager` 装 `ndk;27.0.12077973` + `platforms;android-34` + `build-tools;34.0.0`
（NDK 路径写进 `ANDROID_NDK_HOME`/`NDK_HOME`）→ `rustup target add aarch64-linux-android` →
`npm run android:init`（生成 `src-tauri/gen/android`）→ `npm run prepare:android`
（图标/清单/前端 bundle）→ `tauri android build --apk --debug --target aarch64 --ci --features gepa-lab`。

默认出 **debug APK**（免签名，可直接侧载安装）。要出 release APK 需要 keystore：
把 `inputs.debug` 设成 false，并在仓库 Secrets 里配好签名参数（见下节）。

## 代码签名（尚未启用）

现状：Windows 安装包、免安装包、Android APK **都没有签名**。Windows 首启会出现
SmartScreen「未知发布者」，Android 侧载会提示「来源未知」。这不是遗漏，而是缺证书。

拿到证书后要改三处：

1. **Windows Authenticode**（EV/OV 证书 + `signtool`）—— 在 `release-windows.yml`
   的 `Build NSIS installer` 之后、上传之前加签名步骤，签 `setup.exe`、`guxuanyou.exe`、
   `stock-optimizer-desktop.exe`。证书以 base64 放 Secret（`WINDOWS_CERT_PFX`），
   密码放 `WINDOWS_CERT_PASSWORD`，导入到临时证书存储后 `signtool sign /fd sha256 /tr <时间戳服务器>`。
2. **Tauri 内置签名**（可选，用于 updater）—— `tauri.conf.json` 的
   `plugins.updater.pubkey` + 环境变量 `TAURI_SIGNING_PRIVATE_KEY`。
3. **Android keystore** —— `keytool -genkeypair` 生成 `.jks`，base64 存 Secret；
   构建时写 `gen/android/keystore.properties`，`build.gradle.kts` 里读 `storeFile`/
   `storePassword`/`keyAlias`/`keyPassword`。**keystore 一旦丢失就无法给已安装用户升级**，
   必须离线备份。

在没有证书之前，发布说明里明确写「未签名」是当前唯一诚实做法。
