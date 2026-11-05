# CI 构建与发布

两个 workflow，都在 `.github/workflows/`：

| 文件 | 触发 | 产物 |
| --- | --- | --- |
| `frontend.yml` | push / PR | 前端 735 用例 + 7 个契约脚本 + gp-core 的 fmt/test/clippy（不发布） |
| `release-windows.yml` | 手动 `workflow_dispatch` | Windows NSIS 安装包 → GitHub Release |

## 发 Windows 安装包

Actions → **Release Windows installer** → Run workflow。可填 `tag`，留空则自动用
`v<version>-ci.<run_number>`，并标为预发布。产物名统一为
`guxuanyou_<version>_windows_x64_setup.exe`（NSIS 默认名会把中文 productName 清成空，workflow 里已改名）。

注意事项：

- 安装包**没有** Authenticode 签名，Windows SmartScreen 会提示「未知发布者」。
- 构建走 `desktop/package.json` 的 `build:windows`，会先跑 `prepare:windows`
  （下载 RAG 嵌入模型 + 构建前端 + 准备 Android 资源），再 `tauri build --features gepa-lab`。
- 首次冷构建约 25 分钟（Rust 依赖全量编译），rust-cache 命中后约 10 分钟。

## 已知限制

- `scripts/release-check.ps1` 依赖 PowerShell 与 Windows 打包链路，CI 里不跑；
  本机要发正式版仍用它。
- `.github/workflows/rag-evaluation.yml` 需要外部模型服务，CI 上的失败多为环境缺失。
- 曾经加过「评测/开发镜像」（GHCR）的 workflow 与 Dockerfile，已移除：本项目是 Tauri 桌面 GUI，
  容器里只能装工具链、装不出能跑的应用，留着是死重量。
