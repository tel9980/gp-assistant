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

冲突码保留大写形式，因为前端 `watchlistStore` 早已按这两个字符串判定；改名收益为零、风险不小。

## 迁移下一个模块的配方

1. 在该模块里把签名 `Result<X, String>` 换成 `Result<X, CommandError>`（`use crate::error::{codes, CommandError};`）。
2. 把 `fn sql_error(...) -> String` 这类适配函数改成返回 `CommandError::new(codes::XXX, ...)`。
3. 字面量 `Err(format!(...))` 改成 `CommandError::new(codes::XXX, format!(...))`，需要时 `.with_hint(...)`。
4. 该模块的单元测试里，`err.contains("...")` 改用 `err.code()` / `err.to_string().contains(...)`。
5. 在 `codes` 里加该域的错误码，`docs/error-model.md` 的表格补一行。
6. 前端若需要区分，用 `hasErrorCode`；展示一律 `errorMessage`。

## 验证

- Rust：`cd native/gp-core && cargo test --lib`（不受影响）；`desktop/src-tauri` 只能由 CI 的 `desktop shell (cargo check)` job 编译（本机缺 webkit2gtk/gtk），本地可先 `cargo fmt --all --check` 保证语法。
- 前端：`npm run test:fast`，`src/lib/appError.test.ts` 覆盖四种错误形态。
