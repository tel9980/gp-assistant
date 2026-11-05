import { describe, expect, it } from "vitest";
import { errorMessage, hasErrorCode, toAppError } from "./appError";

describe("toAppError", () => {
  it("keeps a structured backend error intact", () => {
    const error = toAppError({
      code: "watchlist.sqlite",
      message: "watchlist sqlite: disk I/O error",
      hint: "检查磁盘空间后重试",
      details: { revision: 3 },
    });
    expect(error.message).toBe("watchlist sqlite: disk I/O error");
    expect(error.code).toBe("watchlist.sqlite");
    expect(error.hint).toBe("检查磁盘空间后重试");
    expect(error.details).toEqual({ revision: 3 });
    expect(error.fromNative).toBe(true);
  });

  it("unwraps the legacy JSON-string error shape", () => {
    const error = toAppError(JSON.stringify({ code: "WATCHLIST_CONFLICT", revision: 2, message: "watchlist changed" }));
    expect(error.message).toBe("watchlist changed");
    expect(error.code).toBe("WATCHLIST_CONFLICT");
    expect(error.fromNative).toBe(true);
  });

  it("passes plain strings through unchanged", () => {
    const error = toAppError("无法连接自选股存储，请重试。");
    expect(error.message).toBe("无法连接自选股存储，请重试。");
    expect(error.code).toBe("app.unknown");
    expect(error.fromNative).toBe(false);
  });

  it("keeps Error instances and fills a fallback code", () => {
    const error = toAppError(new Error("boom"));
    expect(error.message).toBe("boom");
    expect(error.code).toBe("app.unknown");
  });

  it("never throws on exotic values", () => {
    expect(toAppError(undefined).message).toBe("");
    expect(toAppError(null).message).toBe("");
    expect(toAppError(42).message).toBe("42");
  });
});

describe("helpers", () => {
  it("matches codes across both error shapes", () => {
    expect(hasErrorCode({ code: "watchlist.schema", message: "x" }, "watchlist.schema")).toBe(true);
    expect(hasErrorCode(JSON.stringify({ code: "WATCHLIST_CONFLICT", message: "y" }), "WATCHLIST_CONFLICT")).toBe(true);
    expect(hasErrorCode("plain", "WATCHLIST_CONFLICT")).toBe(false);
  });

  it("errorMessage is the display string for every shape", () => {
    expect(errorMessage({ code: "a", message: "结构体" })).toBe("结构体");
    expect(errorMessage(JSON.stringify({ code: "b", message: "旧格式" }))).toBe("旧格式");
    expect(errorMessage(new Error("实例"))).toBe("实例");
  });
});
