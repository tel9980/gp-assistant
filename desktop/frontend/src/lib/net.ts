// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { RequestOptions, cancelTauriAgentRun } from "./tauri";
import { getTauriInvoke, isTauriRuntime } from "./tauriRuntime";
import type { InvokeFn } from "./tauriRuntime";
import { invokeNativeJob, isNativeJobCommand } from "./nativeJobs";
import { MOBILE_MARKET_REFRESH_INVOKE_TIMEOUT_MS } from "./marketRefresh";
import { tauriRouteHandler } from "./tauri";

export function createTimeoutSignal(timeoutMs?: number, message?: string): { signal: AbortSignal | null; cancel: () => void } {
  if (!Number.isFinite(timeoutMs) || (timeoutMs ?? 0) <= 0 || typeof AbortController === "undefined") return { signal: null, cancel: () => {} };
  const controller = new AbortController();
  const timer = window.setTimeout(() => {
    controller.abort(createRequestAbortError(message || `请求超过 ${Math.round((timeoutMs as number) / 1000)} 秒未返回`, "TimeoutError"));
  }, timeoutMs as number);
  return { signal: controller.signal, cancel: () => window.clearTimeout(timer) };
}

export const abortCleanups = new WeakMap<AbortSignal, () => void>();
export function releaseAbortSignal(signal: AbortSignal | null | undefined): void {
  if (signal) { abortCleanups.get(signal)?.(); abortCleanups.delete(signal); }
}
export function combineAbortSignals(...signals: (AbortSignal | null | undefined)[]): AbortSignal | null {
  const sources = signals.filter(Boolean) as AbortSignal[];
  if (!sources.length || typeof AbortController === "undefined") return sources[0] || null;
  const controller = new AbortController();
  const removers: Array<() => void> = [];
  const dispose = () => { removers.splice(0).forEach(remove => remove()); };
  abortCleanups.set(controller.signal, dispose);
  for (const source of sources) {
    const abort = () => { controller.abort(abortReason(source)); dispose(); };
    if (source.aborted) { abort(); break; }
    source.addEventListener("abort", abort, { once: true });
    removers.push(() => source.removeEventListener("abort", abort));
  }
  return controller.signal;
}

export function abortReason(signal: AbortSignal | null | undefined): Error {
  if (!signal) return new Error("请求已取消");
  const reason = (signal as AbortSignal & { reason?: unknown }).reason;
  if (reason instanceof Error) return reason;
  return createRequestAbortError("请求已取消", "AbortError");
}

export function createRequestAbortError(message: string, name = "AbortError"): Error {
  try {
    return new DOMException(message, name);
  } catch {
    const error = new Error(message);
    error.name = name;
    return error;
  }
}

export function withAbortSignal<T>(promise: Promise<T>, signal: AbortSignal | null): Promise<T> {
  if (!signal) return promise;
  if (signal.aborted) return Promise.reject(abortReason(signal));
  return new Promise<T>((resolve, reject) => {
    const cleanup = () => signal.removeEventListener("abort", abort);
    const abort = () => { cleanup(); reject(abortReason(signal)); };
    signal.addEventListener("abort", abort, { once: true });
    promise.then(value => { cleanup(); resolve(value); }, error => { cleanup(); reject(error); });
  });
}

export async function requestJson<T = unknown>(method: string, url: string, payload?: unknown, headers: Record<string, string> = {}, options: RequestOptions = {}): Promise<T> {
  const timeoutSignal = createTimeoutSignal(options.timeoutMs);
  const signal = combineAbortSignals(options.signal ?? null, timeoutSignal.signal);
  let removeNativeAgentAbortListener: (() => void) | undefined;
  if (signal && isTauriRuntime()) {
    const path = new URL(url, window.location.href).pathname;
    const runId = String(asRecord(payload).run_id || "").trim();
    if ((path === "/api/agent" || path === "/api/agent/stream") && runId) {
      const cancelRun = () => {
        void cancelTauriAgentRun(runId).catch(() => undefined);
      };
      if (signal.aborted) cancelRun();
      else {
        signal.addEventListener("abort", cancelRun, { once: true });
        removeNativeAgentAbortListener = () => signal.removeEventListener("abort", cancelRun);
      }
    }
  }
  try {
    if (signal?.aborted) throw abortReason(signal);
    const tauriResult = await withAbortSignal(requestTauriJson(method, url, payload, signal), signal);
    if (tauriResult.handled) return tauriResult.data as T;
    const request: RequestInit = { method, headers: method === "POST" ? { "Content-Type": "application/json", ...headers } : headers };
    if (signal) request.signal = signal;
    if (payload !== undefined) request.body = JSON.stringify(payload);
    const resp = await fetch(url, request);
    if (!resp.ok) throw Object.assign(new Error(await resp.text() || `HTTP ${resp.status}`), { status: resp.status });
    return (await resp.json()) as T;
  } finally {
    removeNativeAgentAbortListener?.();
    timeoutSignal.cancel();
    releaseAbortSignal(signal);
  }
}

export async function postJson<T = unknown>(url: string, payload?: unknown, options?: RequestOptions): Promise<T> {
  return requestJson<T>("POST", url, payload, {}, options || {});
}

export async function getJson<T = unknown>(url: string, options?: RequestOptions): Promise<T> {
  return requestJson<T>("GET", url, undefined, {}, options || {});
}

export async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  const { signal, cancel } = createTimeoutSignal(timeoutMs, message);
  try {
    return await withAbortSignal(promise, signal);
  } finally {
    cancel();
  }
}

export async function fetchWithTimeout(input: RequestInfo | URL, init: RequestInit, timeoutMs: number, message: string): Promise<Response> {
  const { signal, cancel } = createTimeoutSignal(timeoutMs, message);
  try {
    return await fetch(input, { ...init, signal: signal || undefined });
  } finally {
    cancel();
  }
}

export async function openExternalUrl(rawUrl: string): Promise<void> {
  const url = normalizeExternalUrl(rawUrl);
  if (!url) return;
  const invoke = getTauriInvoke();
  if (invoke) {
    try {
      await invoke("open_external_url", { url });
      return;
    } catch (error) {
      console.warn("open_external_url failed, falling back to window.open", error);
    }
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export function normalizeExternalUrl(rawUrl: string): string {
  try {
    const url = new URL(String(rawUrl || "").trim(), window.location.href);
    return ["http:", "https:"].includes(url.protocol) ? url.toString() : "";
  } catch {
    return "";
  }
}

export function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

export function arrayRecords(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? value.map(asRecord).filter((item) => Object.keys(item).length) : [];
}

export async function requestTauriJson(method: string, url: string, payload?: unknown, signal?: AbortSignal | null): Promise<{ handled: boolean; data?: unknown }> {
  const rawInvoke = getTauriInvoke();
  if (!rawInvoke) return { handled: false };
  const invoke: InvokeFn = <T,>(command: string, args?: Record<string, unknown>): Promise<T> => {
    if (!isNativeJobCommand(command)) return rawInvoke<T>(command, args);
    const deadline = createTimeoutSignal(command === "api_market_refresh" ? MOBILE_MARKET_REFRESH_INVOKE_TIMEOUT_MS : undefined);
    const combined = combineAbortSignals(signal, deadline.signal);
    return invokeNativeJob<T>(rawInvoke, command, args, { signal: combined }).finally(() => { deadline.cancel(); releaseAbortSignal(combined); });
  };
  const parsed = new URL(url, window.location.href);
  const normalizedMethod = String(method || "GET").toUpperCase();
  const handler = tauriRouteHandler(normalizedMethod, parsed.pathname);
  if (handler) return { handled: true, data: await handler({ invoke, parsed, path: parsed.pathname, payload }) };
  if (normalizedMethod === "GET" || normalizedMethod === "POST") throw new Error(`移动端暂不支持该接口：${parsed.pathname}`);
  return { handled: false };
}
