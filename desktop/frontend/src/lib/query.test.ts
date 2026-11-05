import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { state } = vi.hoisted(() => ({
  state: { invoke: undefined as undefined | ((command: string, args?: Record<string, unknown>) => Promise<unknown>) },
}));
const invokeMock = vi.fn();

vi.mock("./tauri", () => ({
  getTauriInvoke: () => state.invoke,
}));

const {
  queryTauri,
  invalidateQuery,
  queryKey,
  queryCacheSize,
  resetQueryCacheForTests,
} = await import("./query");

beforeEach(() => {
  resetQueryCacheForTests();
  invokeMock.mockReset();
  state.invoke = invokeMock;
  vi.useRealTimers();
});

afterEach(() => {
  resetQueryCacheForTests();
});

describe("queryTauri", () => {
  it("merges concurrent identical calls into one IPC", async () => {
    let release: (value: unknown) => void = () => undefined;
    invokeMock.mockImplementation(
      () => new Promise((resolve) => { release = resolve; }),
    );

    const first = queryTauri("api_diagnostics_status", { a: 1, b: 2 });
    const second = queryTauri("api_diagnostics_status", { b: 2, a: 1 });
    expect(invokeMock).toHaveBeenCalledTimes(1);

    release({ ok: true });
    await expect(first).resolves.toEqual({ ok: true });
    await expect(second).resolves.toEqual({ ok: true });
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("does not merge different arguments", async () => {
    invokeMock.mockResolvedValue(1);
    await queryTauri("api_diagnostics_status", { a: 1 });
    await queryTauri("api_diagnostics_status", { a: 2 });
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("caches within the TTL and refetches after it", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000);
    invokeMock.mockResolvedValueOnce("first").mockResolvedValueOnce("second");

    await expect(queryTauri("api_status", undefined, { ttlMs: 500 })).resolves.toBe("first");
    await expect(queryTauri("api_status", undefined, { ttlMs: 500 })).resolves.toBe("first");
    expect(invokeMock).toHaveBeenCalledTimes(1);

    vi.setSystemTime(2_000);
    await expect(queryTauri("api_status", undefined, { ttlMs: 500 })).resolves.toBe("second");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("does not cache when the TTL is zero", async () => {
    invokeMock.mockResolvedValueOnce("first").mockResolvedValueOnce("second");
    await expect(queryTauri("api_status")).resolves.toBe("first");
    await expect(queryTauri("api_status")).resolves.toBe("second");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("force bypasses a warm cache but refreshes it", async () => {
    invokeMock.mockResolvedValueOnce("old").mockResolvedValueOnce("new");
    await queryTauri("api_status", undefined, { ttlMs: 10_000 });
    await expect(queryTauri("api_status", undefined, { ttlMs: 10_000, force: true })).resolves.toBe("new");
    await expect(queryTauri("api_status", undefined, { ttlMs: 10_000 })).resolves.toBe("new");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("invalidates by command name and by prefix only", async () => {
    invokeMock.mockResolvedValue("x");
    await queryTauri("api_status", undefined, { ttlMs: 10_000 });
    await queryTauri("api_status_detail", undefined, { ttlMs: 10_000 });
    expect(queryCacheSize()).toBe(2);

    expect(invalidateQuery("api_status")).toBe(1);
    expect(queryCacheSize()).toBe(1);

    expect(invalidateQuery()).toBe(1);
    expect(queryCacheSize()).toBe(0);
  });

  it("normalizes native structured errors into AppError", async () => {
    invokeMock.mockRejectedValue({
      code: "research.not_found",
      message: "research thread not found",
      hint: "刷新列表",
      details: { thread_id: "t1" },
    });

    await expect(queryTauri("api_research_thread")).rejects.toMatchObject({
      code: "research.not_found",
      message: "research thread not found",
      hint: "刷新列表",
      details: { thread_id: "t1" },
      fromNative: true,
    });
  });

  it("does not cache failures", async () => {
    invokeMock.mockRejectedValueOnce(new Error("boom")).mockResolvedValueOnce("ok");
    await expect(queryTauri("api_status", undefined, { ttlMs: 10_000 })).rejects.toThrow("boom");
    await expect(queryTauri("api_status", undefined, { ttlMs: 10_000 })).resolves.toBe("ok");
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("rejects without IPC when the signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(queryTauri("api_status", undefined, { signal: controller.signal })).rejects.toMatchObject({
      name: "AbortError",
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("rejects when the Tauri runtime is missing", async () => {
    state.invoke = undefined;
    await expect(queryTauri("api_status")).rejects.toMatchObject({ code: "app.unknown" });
  });

  it("builds stable cache keys", () => {
    expect(queryKey("api_status", { b: 1, a: 2 })).toBe('api_status?{"a":2,"b":1}');
    expect(queryKey("api_status")).toBe("api_status");
    expect(queryKey("api_status", { list: [{ y: 1, x: 2 }] })).toBe('api_status?{"list":[{"x":2,"y":1}]}');
  });
});
