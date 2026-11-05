//! 统一的命令错误类型。
//!
//! 背景：此前所有 `#[tauri::command]` 都返回 `Result<T, String>`，前端拿到的是
//! 一段夹着底层报错的字符串（`"watchlist sqlite: disk I/O error"`），无法区分
//! 「用户填错了」「数据文件损坏」「网络不通」，也没法按类型决定要不要重试。
//!
//! `CommandError` 序列化成 `{ code, message, hint }`：
//! - `code`：稳定的小写点分标识（`watchlist.sqlite`），前端可据此分支或上报；
//! - `message`：仍是原来那段中文/英文说明，`Display` 与旧字符串逐字一致，
//!   所以日志、测试断言、前端 `alert` 的观感都不变；
//! - `hint`：可选的可执行建议（「先备份 data 目录」「检查代理」）。
//!
//! 迁移是逐域进行的：未迁移的模块继续返回 `String`，`From<String>` 保证两边
//! 可以互相 `?`，不存在一次性全量改造的风险。

use serde::{Serialize, Serializer};
use std::fmt;

/// 命令错误码常量。新增时保持「域.原因」两段式，避免前端出现上百个碎片码。
pub mod codes {
    pub const WATCHLIST_IO: &str = "watchlist.io";
    pub const WATCHLIST_SQLITE: &str = "watchlist.sqlite";
    pub const WATCHLIST_SCHEMA: &str = "watchlist.schema";
    pub const WATCHLIST_INPUT: &str = "watchlist.input";
    pub const WATCHLIST_INTERNAL: &str = "watchlist.internal";
    pub const RESEARCH_IO: &str = "research.io";
    pub const RESEARCH_SQLITE: &str = "research.sqlite";
    pub const RESEARCH_INPUT: &str = "research.input";
    pub const RESEARCH_NOT_FOUND: &str = "research.not_found";
    pub const RESEARCH_INDEX_STALE: &str = "research.index_stale";
    pub const RESEARCH_PACK_INVALID: &str = "research.pack_invalid";
    pub const RESEARCH_INTERNAL: &str = "research.internal";

    /// 用户备份（手动加密备份 / 恢复）。
    pub const BACKUP_IO: &str = "backup.io";
    pub const BACKUP_CRYPTO: &str = "backup.crypto";
    pub const BACKUP_SQLITE: &str = "backup.sqlite";
    pub const BACKUP_SCHEMA: &str = "backup.schema";
    pub const BACKUP_INPUT: &str = "backup.input";
    pub const BACKUP_INTERNAL: &str = "backup.internal";

    /// 进化（策略 / 画像 / 复盘）。
    pub const EVOLUTION_IO: &str = "evolution.io";
    pub const EVOLUTION_SQLITE: &str = "evolution.sqlite";
    pub const EVOLUTION_SCHEMA: &str = "evolution.schema";
    pub const EVOLUTION_INPUT: &str = "evolution.input";
    pub const EVOLUTION_INTERNAL: &str = "evolution.internal";
    pub const MARKET_IO: &str = "market.io";
    pub const MARKET_NETWORK: &str = "market.network";
    pub const MARKET_PROVIDER: &str = "market.provider";
    pub const MARKET_INPUT: &str = "market.input";
    pub const MARKET_INTERNAL: &str = "market.internal";
    pub const WATCHLIST_CONFLICT: &str = "WATCHLIST_CONFLICT";
    pub const WATCHLIST_OPERATION_REUSED: &str = "WATCHLIST_OPERATION_REUSED";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    code: &'static str,
    message: String,
    hint: Option<String>,
    details: Option<serde_json::Value>,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            details: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// 结构化附加信息（如冲突时的当前 revision），前端可据此做精确恢复。
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn details(&self) -> Option<&serde_json::Value> {
        self.details.as_ref()
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }
}

/// 错误值的字符串视图就是它的 `message`。
///
/// 迁移期不少代码把错误当字符串用（`err.to_ascii_lowercase()`、传 `&str` 参数、
/// `err.contains("...")` 断言）。Deref 到 `str` 让这些用法继续成立，
/// 同时不改变序列化契约（前端仍拿到 `{ code, message, ... }`）。
impl std::ops::Deref for CommandError {
    type Target = str;

    fn deref(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl Serialize for CommandError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CommandError", 4)?;
        state.serialize_field("code", self.code)?;
        state.serialize_field("message", &self.message)?;
        match &self.hint {
            Some(hint) => state.serialize_field("hint", hint)?,
            None => state.serialize_field("hint", &Option::<String>::None)?,
        }
        match &self.details {
            Some(details) => state.serialize_field("details", details)?,
            None => state.serialize_field("details", &Option::<serde_json::Value>::None)?,
        }
        state.end()
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        // 未迁移的模块只给出自由文本；用 `internal` 兜底，前端仍可统一展示。
        Self::new(codes::WATCHLIST_INTERNAL, message)
    }
}

impl From<CommandError> for String {
    /// 迁移期桥接：未改造的调用方仍按 `Result<_, String>` 工作，
    /// `?` / `map_err` 到这里取回原始文案，行为与迁移前一致。
    fn from(error: CommandError) -> Self {
        error.message
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::new(codes::WATCHLIST_IO, error.to_string())
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new(codes::WATCHLIST_SQLITE, error.to_string())
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(codes::WATCHLIST_INPUT, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_legacy_message() {
        let error = CommandError::new(codes::WATCHLIST_SQLITE, "watchlist sqlite: disk I/O error");
        assert_eq!(error.to_string(), "watchlist sqlite: disk I/O error");
    }

    #[test]
    fn serializes_code_message_hint() {
        let error = CommandError::new(codes::WATCHLIST_SCHEMA, "unsupported schema version 9")
            .with_hint("备份后删除 watchlist.db 重建");
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["code"], "watchlist.schema");
        assert_eq!(value["message"], "unsupported schema version 9");
        assert_eq!(value["hint"], "备份后删除 watchlist.db 重建");
    }

    #[test]
    fn hint_serializes_as_null_when_absent() {
        let value = serde_json::to_value(CommandError::new(codes::WATCHLIST_IO, "boom"))
            .expect("serialize");
        assert!(value["hint"].is_null());
    }

    #[test]
    fn conflict_carries_revision_details() {
        let error = CommandError::new(codes::WATCHLIST_CONFLICT, "watchlist changed")
            .with_details(serde_json::json!({ "revision": 7 }));
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["code"], "WATCHLIST_CONFLICT");
        assert_eq!(value["details"]["revision"], 7);
    }

    #[test]
    fn string_conversion_preserves_text() {
        let error = CommandError::from("legacy message".to_string());
        assert_eq!(error.message(), "legacy message");
    }
}
