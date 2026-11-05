// 原生错误归一化。
//
// 后端 `#[tauri::command]` 的错误有两种形态并存（迁移期）：
//   1. 旧风格：一段字符串（有时是 JSON 字符串，如 `{"code":"WATCHLIST_CONFLICT",...}`）；
//   2. 新风格：`CommandError` 序列化成 `{ code, message, hint, details }`。
// 并且 invoke 失败时还可能是 `Error` 实例或任意值。
//
// 统一从 `toAppError` 进：拿到带 `code`/`hint` 的 `Error`，`message` 与改造前
// 的展示文案逐字一致，所以 UI 层不需要一起改。

export type AppErrorInfo = {
  code: string;
  message: string;
  hint?: string;
  details?: unknown;
};

export type AppError = Error & {
  code: string;
  hint?: string;
  details?: unknown;
  /** 错误来自后端结构化错误（而不是本地抛出的 Error）。 */
  fromNative: boolean;
};

const FALLBACK_CODE = "app.unknown";

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null ? (value as Record<string, unknown>) : null;
}

function parseLegacyJson(text: string): Record<string, unknown> | null {
  const trimmed = text.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    return asRecord(JSON.parse(trimmed));
  } catch {
    return null;
  }
}

/** 把任意错误值转成统一的 `AppError`，永远不会抛。 */
export function toAppError(error: unknown): AppError {
  const record = asRecord(error);
  if (record && typeof record.message === "string") {
    const result = new Error(record.message) as AppError;
    result.code = typeof record.code === "string" && record.code ? record.code : FALLBACK_CODE;
    if (typeof record.hint === "string") result.hint = record.hint;
    if (record.details !== undefined) result.details = record.details;
    result.fromNative = true;
    return result;
  }
  if (error instanceof Error) {
    const legacy = parseLegacyJson(error.message);
    if (legacy && typeof legacy.message === "string") {
      const result = new Error(legacy.message) as AppError;
      result.code = typeof legacy.code === "string" && legacy.code ? legacy.code : FALLBACK_CODE;
      if (typeof legacy.hint === "string") result.hint = legacy.hint;
      if (legacy.details !== undefined) result.details = legacy.details;
      result.fromNative = true;
      return result;
    }
    const result = error as AppError;
    result.code ??= FALLBACK_CODE;
    result.fromNative = Boolean((error as { fromNative?: boolean }).fromNative);
    return result;
  }
  if (typeof error === "string") {
    const legacy = parseLegacyJson(error);
    const message = legacy && typeof legacy.message === "string" ? legacy.message : error;
    const result = new Error(message) as AppError;
    result.code = legacy && typeof legacy.code === "string" && legacy.code ? legacy.code : FALLBACK_CODE;
    if (legacy && typeof legacy.hint === "string") result.hint = legacy.hint;
    if (legacy && legacy.details !== undefined) result.details = legacy.details;
    result.fromNative = Boolean(legacy);
    return result;
  }
  const result = new Error(String(error ?? "")) as AppError;
  result.code = FALLBACK_CODE;
  result.fromNative = false;
  return result;
}

/** UI 展示用的一句话，兼容所有历史形态。 */
export function errorMessage(error: unknown): string {
  return toAppError(error).message;
}

/** 后端错误码判定；旧 JSON 字符串与新对象都能命中。 */
export function hasErrorCode(error: unknown, code: string): boolean {
  return toAppError(error).code === code;
}
