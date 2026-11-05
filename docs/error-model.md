# 命令错误模型（`CommandError`）

## 为什么改

历史上所有 `#[tauri::command]` 都返回 `Result<T, String>`，前端 `invoke` 失败时拿到的是一段字符串：

```
watchlist sqlite: disk I/O error
```

分不清「用户输入错了」「本地数据文件损坏」「网盘/接口不通」，界面只能原样弹字；想按类型决定「要不要重试」「要不要引导去修数据」都做不到。个别地方（自选股冲突）甚至把 JSON 塞进字符串里，靠前端 `JSON.parse` 反解。

现在有了统一错误类型 `CommandError`（`desktop/src-tauri/src/error.rs`），序列化成：

```json
{ "code": "watchlist.sqlite", "message": "watchlist sqlite: disk I/O error", "hint": "...", "details": null }
```

- `code`：稳定的「域.原因」标识，前端可分支、可上报；
- `message`：**与旧字符串逐字一致**，日志、测试断言、界面文案都不用改；
- `hint`：可选的可执行建议；
- `details`：可选结构化附加信息（如冲突时的当前 `revision`）。

## 兼容性（迁移期的关键设计）

迁移是逐域进行的，两种形态必须能共存：

| 方向 | 机制 | 说明 |
| --- | --- | --- |
| `String` → `CommandError` | `impl From<String> for CommandError` | 旧函数用 `?` 进入新函数自动转换，文案保留，`code` 落到域级兜底值 |
| `CommandError` → `String` | `impl From<CommandError> for String` | 新函数被旧调用方 `?`/`map_err` 消费，行为与改造前一致 |
| 前端任意形态 → `AppError` | `desktop/frontend/src/lib/appError.ts` 的 `toAppError` | 字符串、JSON 字符串、`Error`、结构化对象都能归一化 |

`CommandError` 还实现了 `Deref<Target = str>`（视图即 `message`），迁移期 `err.to_ascii_lowercase()`、`err.contains("...")`、把 `&err` 传给 `&str` 参数都继续可用；但**新代码请用 `code()` / `errorMessage()`**。

前端不要直接 `String(error)`（结构化错误会变成 `[object Object]`），用：

```ts
import { errorMessage, hasErrorCode } from "./appError";

publish("error", errorMessage(error));           // 展示
if (hasErrorCode(error, "WATCHLIST_CONFLICT")) { /* 重新对齐 */ }
```

## 已迁移

| 模块 | 错误码 | 备注 |
| --- | --- | --- |
| `desktop/src-tauri/src/watchlist.rs` | `watchlist.io` / `watchlist.sqlite` / `watchlist.schema` / `watchlist.input` / `watchlist.internal`，冲突与重放沿用 `WATCHLIST_CONFLICT` / `WATCHLIST_OPERATION_REUSED` | 自选股 SQLite 全链路；冲突另带 `details.revision` |
| `desktop/src-tauri/src/evolution.rs` | `evolution.io` / `evolution.sqlite` / `evolution.schema` / `evolution.input` / `evolution.internal` | 策略/画像/复盘域 39 处签名；新增 `sql_error`（rusqlite）与 `io_error`（`std::fs`）两个辅助函数，`.map_err(\|error\| ce(error))` 的写法统一替换为对应辅助函数 |
| `desktop/src-tauri/src/user_backup.rs` | `backup.io` / `backup.crypto` / `backup.sqlite` / `backup.schema` / `backup.input` / `backup.internal` | 用户备份域 38 处签名；`sql_error` / `io_error` 两个辅助函数改为返回 `CommandError`（带 `backup.sqlite` / `backup.io`），口令校验落 `backup.crypto`、格式版本落 `backup.schema` |
| `desktop/src-tauri/src/hk_market.rs` | `market.network` / `market.provider` / `market.io` / `market.input` / `market.internal` | 港股域 50 处签名；`merge_hk_observation_update` / `merge_hk_financial_refresh` 仍收 `Result<_, String>`，在调用点 `.map_err(String::from)`；legacy 导入闭包（`import_legacy_json`）也必须回 String |
| `desktop/src-tauri/src/jobs.rs` | `jobs.input` / `jobs.io` / `jobs.internal` | 作业队列 27 处签名；`Manager::execute` 的 `Fut: Future<Output = Result<Value, CommandError>>`、`end()` 收 `Result<Value, CommandError>`，`job_cancelled` / `job_timed_out` / `job_native_panicked` 走 `jobs.internal`，`job_schema_unsupported` 走 `jobs.input` |
| `desktop/src-tauri/src/screening.rs` | `screening.input` / `screening.io` / `screening.internal` | 选股/回测域 40 处签名；`adaptive_screen_with_timeout<T, E>` 泛型化为 `E: Into<CommandError>`；「没有本地行情数据」落 `screening.io` |
| `desktop/src-tauri/src/observe.rs` | `observe.input` / `observe.internal` | 观察页域 24 处签名；请求缺少股票代码落 `observe.input` |
| `desktop/src-tauri/src/credentials.rs` | `credentials.input` | 凭据引用/市场数据 provider 标识校验；密钥仍只在内存与 OS 密钥链之间流转 |
| `desktop/src-tauri/src/durability.rs` | `durability.io` / `durability.sqlite` | 崩溃安全落盘：目录创建走 `durability.io`，SQLite 快照校验走 `durability.sqlite` |
| `desktop/src-tauri/src/agent_ledger.rs` | `agent_ledger.io` / `agent_ledger.sqlite` | Agent 运行账本；打开数据库失败落 `agent_ledger.sqlite`，建目录失败落 `agent_ledger.io` |
| `desktop/src-tauri/src/external_data/store.rs` | `external_store.io` / `external_store.sqlite` / `external_store.internal` | 外部数据仓 31 处签名；`import_legacy_json` 的闭包仍回 `String`，调用点 `.map_err(CommandError::from)` |
| `desktop/src-tauri/src/gepa_lab.rs` | `gepa_lab.io` / `gepa_lab.internal` | GEPA 实验域 14 处签名 |
| `desktop/src-tauri/src/prompt_upgrade.rs` | `prompt_upgrade.io` / `prompt_upgrade.internal` | 提示词升级域 15 处签名 |
| `desktop/src-tauri/src/research_recovery.rs` | `research_recovery.io` / `research_recovery.sqlite` / `research_recovery.internal` | 研究库恢复 19 处签名；快照文件读写走 io，校验走 sqlite |
| `desktop/src-tauri/src/news_rag.rs` | `news_rag.io` / `news_rag.input` / `news_rag.internal` | 新闻 RAG 23 处签名 |
| `desktop/src-tauri/src/diagnostics.rs` | `diagnostics.internal` | 诊断快照 14 处签名 |
| `desktop/src-tauri/src/research_import.rs` | `research_import.internal` | 研究资料导入 12 处签名（含 URL 抓取的私网拦截） |
| `desktop/src-tauri/src/agent_harness.rs` | `agent_harness.internal` | Agent 运行时外壳 10 处签名；`ModelCallFailure` 仍在内部流转，只在命令边界转成 `CommandError` |
| `desktop/src-tauri/src/rag_pack.rs` | `rag_pack.internal` | RAG 知识包 25 处签名（打包/校验/装载） |
| `desktop/src-tauri/src/core_api.rs` | — | 14 处签名；统一走 `run_core`，内部把 `Result<_, String>` 用 `CommandError::from` 转过来 |
| `desktop/src-tauri/src/workspace.rs` | `workspace.internal` | 工作区存储 6 处签名；`err()` 辅助函数直接返回 `CommandError` |
| `desktop/src-tauri/src/eval_replay.rs` | `eval_replay.internal` | 评测回放 7 处签名 |
| `desktop/src-tauri/src/external_data/mod.rs` | `external_data.internal` | 外部数据命令层 9 处签名 |
| `desktop/src-tauri/src/stock_research_runtime.rs` | `stock_research.internal` | 研究报告运行时 14 处签名 |
| `desktop/src-tauri/src/market.rs`（含 `observe.rs`、`screening.rs`、`jobs.rs`、`hk_market.rs`、`rig_runtime.rs`、`external_data/mod.rs` 接缝） | `market.network` / `market.provider` / `market.io` / `market.input` / `market.internal` | 行情域 72 处签名；`ce()` 按消息关键词分类到 network/provider/io/input，其余落 internal。**这是被最多模块共享的域**：`with_market_refresh_permit` / `with_heavy_network_permit` 改成泛型 `E: From<String>`，`screening.rs` 在闭包内 `.map_err(String::from)` 回到 String |
| `desktop/src-tauri/src/research.rs`（含 `research_recovery.rs`、`jobs.rs`、`sentiment.rs` 的接缝） | `research.io` / `research.sqlite` / `research.input` / `research.not_found` / `research.index_stale` / `research.pack_invalid` / `research.internal` | 研究库 90 处签名；`research.not_found` 已接到 `NewsRagPanel`：会话不存在时从列表摘掉并提示，而不是让用户反复点「重试加载会话」 |

冲突码保留大写形式，因为前端 `watchlistStore` 早已按这两个字符串判定；改名收益为零、风险不小。

`research.rs` 的迁移方式值得复用：模块内所有 `Err(format!(...))` / `.map_err(|e| format!(...))` 统一走一个私有 `fn ce(message: impl Display) -> CommandError`（落 `research.internal` 兜底，文案不变），只在**能区分处理**的地方换成精确码（`research.not_found`、`research.input`、`research.pack_invalid`、`research.sqlite`、`research.index_stale`）。这样一次迁移 90 个签名也不会把文案改乱。

同时 `with_app_store` / `with_app_store_snapshot` / `with_app_store_at_generation` / `with_recovered_app_database` / `with_shared_app_database` / `with_exclusive_app_database` 都改成了泛型 `E: From<String> + From<CommandError>`：老调用方（闭包返回 `Result<_, String>`）不用改，新调用方（`CommandError`）也能直接透传。

## 迁移下一个模块的配方

1. 在该模块里把签名 `Result<X, String>` 换成 `Result<X, CommandError>`（`use crate::error::{codes, CommandError};`）。
2. 把 `fn sql_error(...) -> String` 这类适配函数改成返回 `CommandError::new(codes::XXX, ...)`。
3. 字面量 `Err(format!(...))` 改成 `CommandError::new(codes::XXX, format!(...))`，需要时 `.with_hint(...)`。
4. 该模块的单元测试里，`err.contains("...")` 改用 `err.code()` / `err.to_string().contains(...)`。
5. 在 `codes` 里加该域的错误码，`docs/error-model.md` 的表格补一行。
6. 前端若需要区分，用 `hasErrorCode`；展示一律 `errorMessage`。

## 验证

- Rust：`cd native/gp-core && cargo test --lib`（不受影响）；`desktop/src-tauri` 本地也能编译了 —— 装上 `libwebkit2gtk-4.1-dev libgtk-3-dev` 后 `cargo check --all-targets --features gepa-lab` 约 2m41s（增量秒级），`cargo test --lib` 约 50s。注意 `stock_research_runtime::tests` 里有 2 条 `hash_mismatch` 用例在干净树上也红（fixture 的 SHA256 与当前导出逻辑不一致），与本模型无关。
- 前端：`npm run test:fast`，`src/lib/appError.test.ts` 覆盖四种错误形态。
