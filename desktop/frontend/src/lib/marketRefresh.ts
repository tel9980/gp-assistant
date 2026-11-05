// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { clampInt, expectedMarketQuoteDateCompact, localTradeDateCompact, uniqueNotes } from "./format";
import { invokeNativeJob } from "./nativeJobs";
import { asRecord, combineAbortSignals, createRequestAbortError, createTimeoutSignal, releaseAbortSignal } from "./net";
import { financialSnapshotPayload, loadBundledScreenSnapshot } from "./snapshot";
import { MarketRefreshLogEntry, MarketRefreshOptions } from "./tauri";
import { InvokeFn, getTauriListen, isMobileTauriRuntime, isTauriRuntime } from "./tauriRuntime";

export const OBSERVE_PREFETCH_TIMEOUT_MS = 5000;
export const MOBILE_MARKET_REFRESH_INVOKE_TIMEOUT_MS = 120000;
export const MARKET_REFRESH_HISTORY_PREFETCH_LIMIT = 80;
export let marketRefreshInFlight: Promise<Record<string, unknown>> | null = null;

export function defaultCachePolicy(): Record<string, unknown> {
  return { mode: "light", max_bytes: 209715200, daily_days: 500, minute_days: 3 };
}

export function buildAndroidNetworkOptions(): Record<string, unknown> {
  return {
    proxy_mode: localStorage.getItem("stock-optimizer-proxy-mode") || "system",
    proxy_url: localStorage.getItem("stock-optimizer-proxy-url") || "",
    android_short_sources: isMobileTauriRuntime(),
  };
}

export function withAndroidNetworkOptions<T extends Record<string, unknown>>(payload: T): T & Record<string, unknown> {
  if (!isTauriRuntime()) return payload;
  return { ...payload, ...buildAndroidNetworkOptions() };
}

export function refreshTauriMarketData(invoke: InvokeFn, options: MarketRefreshOptions = {}): Promise<Record<string, unknown>> {
  if (marketRefreshInFlight) return marketRefreshInFlight;
  marketRefreshInFlight = refreshTauriMarketDataInternal(invoke, options).finally(() => {
    marketRefreshInFlight = null;
  });
  return marketRefreshInFlight;
}

export async function refreshTauriMarketDataInternal(invoke: InvokeFn, options: MarketRefreshOptions = {}): Promise<Record<string, unknown>> {
  const logs = createMarketRefreshLogger(options.onLog);
  const lifecycle = new AbortController();
  const visibilityChanged = () => { if (typeof document !== "undefined" && document.visibilityState === "hidden") lifecycle.abort(); };
  if (typeof document !== "undefined") document.addEventListener?.("visibilitychange", visibilityChanged);
  visibilityChanged();
  const unlisten = await listenMarketRefreshLogs(logs).catch((error) => {
    logs(`无法监听 Rust 刷新日志：${(error as Error).message}`, "warn");
    return undefined;
  });
  try {
    logs("开始通过 Tauri/Rust 刷新股票池。", "info");
    const financialSnapshot = await loadBundledScreenSnapshot();
    const baseOptions = sanitizeMarketRefreshOptions(options);
    const maxLoops = clampInt(Number(baseOptions.max_refresh_loops || 512), 1, 2048, 512);
    let batchStart = clampInt(baseOptions.batch_start, 0, 100000, 0);
    const batchCount = clampInt(baseOptions.batch_count, 1, 1000, MOBILE_TENCENT_BATCHES_PER_STEP);
    const refreshRequestId = typeof baseOptions.request_id === "string" && baseOptions.request_id.trim()
      ? baseOptions.request_id.trim()
      : `cn-market-${globalThis.crypto?.randomUUID?.() || Date.now()}`;
    let aggregate: Record<string, unknown> | null = null;
    let lastData: Record<string, unknown> | null = null;
    let financialSnapshotSent = false;

    for (let loop = 0; loop < maxLoops; loop += 1) {
      if (lifecycle.signal.aborted || options.signal?.aborted) throw createRequestAbortError("行情刷新已暂停；已提交的缓存保留。", "AbortError");
      logs(`请求行情刷新批次窗口：从第 ${batchStart + 1} 批开始，每轮 ${batchCount} 批。`, "info");
      const payload = withAndroidNetworkOptions({
        ...defaultCachePolicy(),
        ...baseOptions,
        market: "cn",
        domain: "quote",
        request_id: refreshRequestId,
        batch_start: batchStart,
        batch_count: batchCount,
        prefetch_history_limit: clampInt(
          Number(baseOptions.prefetch_history_limit),
          0,
          MARKET_REFRESH_HISTORY_PREFETCH_LIMIT,
          MARKET_REFRESH_HISTORY_PREFETCH_LIMIT,
        ),
        financial_snapshot: !financialSnapshotSent ? financialSnapshotPayload(financialSnapshot) : null,
        scan_candidates: true,
        use_previous_close: shouldUsePreviousCloseForMobileRefresh(),
      });
      if (payload.financial_snapshot) financialSnapshotSent = true;
      const deadline = createTimeoutSignal(MOBILE_MARKET_REFRESH_INVOKE_TIMEOUT_MS);
      const signal = combineAbortSignals(deadline.signal, lifecycle.signal, options.signal);
      let data: Record<string, unknown>;
      try { data = await invokeNativeJob<Record<string, unknown>>(invoke, "api_market_refresh", { payload }, { signal }); }
      finally { deadline.cancel(); releaseAbortSignal(signal); }
      lastData = data;
      aggregate = mergeMarketRefreshResult(aggregate, data);
      logMarketRefreshBatchResult(logs, data, aggregate);

      if (data.done || data.stopped_early || baseOptions.mode === "single") break;
      const nextStart = Number(data.next_batch_start);
      if (!Number.isFinite(nextStart) || nextStart <= batchStart) {
        aggregate = { ...(aggregate || {}), stopped_early: true, stop_reason: "stalled_batches", done: true };
        logs("批次游标没有前进，已停止刷新以避免重复请求。", "error");
        break;
      }
      batchStart = nextStart;
    }

    if (!lastData) throw new Error("Tauri/Rust 行情刷新没有返回任何批次结果。");
    if (aggregate && !aggregate.done && !aggregate.stopped_early) {
      aggregate = { ...aggregate, stopped_early: true, stop_reason: "loop_guard", done: true };
      logs("刷新循环达到安全上限，已停止并保留当前已落盘缓存。", "warn");
    }

    const result: Record<string, unknown> = { ...lastData, refresh_result: aggregate || lastData };
    const notes = mobileRefreshNotes(result, "已通过 Tauri/Rust 联网更新股票池。");
    logs(notes.join(" "), aggregate?.stopped_early ? "warn" : "success");

    let validation: Record<string, unknown> | null = null;
    if (options.validate_after_write !== false) {
      validation = await validateTauriMarketCache(invoke, logs).catch((error) => {
        logs(`刷新后校验失败：${(error as Error).message}`, "error");
        return { ok: false, error: (error as Error).message };
      });
    }

    return {
      ...result,
      validation,
      notes: uniqueNotes([...(Array.isArray(result.notes) ? result.notes as string[] : []), ...notes, ...validationNotes(validation)]),
    };
  } finally {
    if (typeof document !== "undefined") document.removeEventListener?.("visibilitychange", visibilityChanged);
    unlisten?.();
  }
}

export function mergeMarketRefreshResult(previous: Record<string, unknown> | null, result: Record<string, unknown>): Record<string, unknown> {
  const totalBatches = Number(result.total_batches || previous?.total_batches || 0);
  const nextBatchStart = Number(result.next_batch_start || 0);
  const failed = Number(previous?.failed_batches || 0) + Number(result.failed_batches || 0);
  const empty = Number(previous?.empty_batches || 0) + Number(result.empty_batches || 0);
  const fetched = Number(previous?.fetched || 0) + Number(result.fetched || 0);
  const processed = Number(previous?.processed_codes || 0) + Number(result.processed_codes || 0);
  const previousErrors = Array.isArray(previous?.error_samples) ? previous.error_samples : [];
  const resultErrors = Array.isArray(result.error_samples) ? result.error_samples : [];
  return {
    requested: Number(result.requested || result.total_candidates || previous?.requested || 0),
    total_candidates: Number(result.total_candidates || previous?.total_candidates || result.requested || 0),
    fetched,
    preserved: Number(result.preserved || previous?.preserved || 0),
    failed_batches: failed,
    empty_batches: empty,
    stopped_early: Boolean(result.stopped_early || previous?.stopped_early),
    stop_reason: result.stop_reason || previous?.stop_reason || null,
    batch_start: Number(result.batch_start || 0),
    batch_count: Number(result.batch_count || 0),
    next_batch_start: nextBatchStart,
    total_batches: totalBatches,
    completed_batches: Math.min(totalBatches || nextBatchStart, nextBatchStart),
    done: Boolean(result.done || result.stopped_early || (totalBatches > 0 && nextBatchStart >= totalBatches)),
    processed_codes: processed,
    error_samples: [...previousErrors, ...resultErrors].filter(Boolean).slice(-8),
  };
}

export function logMarketRefreshBatchResult(
  log: (message: string, tone?: string) => void,
  result: Record<string, unknown>,
  aggregate: Record<string, unknown> | null,
): void {
  const total = Number(result.total_batches || aggregate?.total_batches || 0);
  const start = Number(result.batch_start || 0) + 1;
  const next = Number(result.next_batch_start || start);
  const fetched = Number(result.fetched || 0);
  const preserved = Number(result.preserved || 0);
  const failed = Number(result.failed_batches || 0);
  const empty = Number(result.empty_batches || 0);
  const tone = failed > 0 || empty > 0 || result.stopped_early ? "warn" : "info";
  log(`批次 ${start}-${next}/${total || "?"} 返回：新增 ${fetched} 只，保留 ${preserved} 只，空批次 ${empty}，失败 ${failed}。`, tone);
}
export function createMarketRefreshLogger(onLog?: (entry: MarketRefreshLogEntry) => void) {
  return (message: string, tone = "info") => {
    if (!message) return;
    onLog?.({ time: new Date().toLocaleTimeString("zh-CN"), message, tone });
  };
}

export function sanitizeMarketRefreshOptions(options: MarketRefreshOptions): Record<string, unknown> {
  const payload: Record<string, unknown> = { ...options };
  delete payload.onLog;
  delete payload.signal;
  if (!payload.max_candidates) payload.max_candidates = MOBILE_TENCENT_MAX_CANDIDATES;
  if (!payload.batch_count) payload.batch_count = MOBILE_TENCENT_BATCHES_PER_STEP;
  if (!payload.max_failed_batches) payload.max_failed_batches = 3;
  delete payload.validate_after_write;
  return payload;
}

export async function validateTauriMarketCache(
  invoke: InvokeFn,
  log?: (message: string, tone?: string) => void,
): Promise<Record<string, unknown>> {
  log?.("正在回读 Tauri/Rust 本地行情缓存。", "info");
  const record = asRecord(await invoke("core_mobile_market_data_read"));
  if (!record.exists) {
    const notes = Array.isArray(record.notes) ? record.notes.map(String).filter(Boolean) : [];
    throw new Error(notes[0] || "本地行情缓存不存在，请先刷新股票池。");
  }
  const data = asRecord(record.data);
  if (!Object.keys(data).length) throw new Error("本地行情缓存回传为空，无法校验。");
  const summary = asRecord(await invoke("core_validate_data_source", { payload: data }));
  const stockCount = Number(summary.stock_count || record.stock_count || 0);
  const historyCount = Number(summary.history_count || 0);
  log?.(`本地行情缓存校验通过：${stockCount} 只股票，历史序列 ${historyCount} 组，落盘 ${record.bytes || 0} B。`, "success");
  return {
    ok: true,
    stock_count: stockCount,
    history_count: historyCount,
    bytes: record.bytes || 0,
    path: record.path || "",
    generated_at: record.generated_at || null,
    summary,
  };
}

export function validationNotes(validation: Record<string, unknown> | null): string[] {
  if (!validation) return [];
  if (validation.ok === false) return [`行情缓存已刷新，但回读校验失败：${validation.error || "unknown"}`];
  return [`行情缓存已落盘并通过校验：${validation.stock_count || 0} 只股票，${validation.bytes || 0} B。`];
}

export async function listenMarketRefreshLogs(log: (message: string, tone?: string) => void): Promise<(() => void) | undefined> {
  const listen = getTauriListen();
  if (!listen) return undefined;
  return listen("market-refresh-log", (event) => {
    const payload = asRecord(asRecord(event).payload);
    const message = formatRustMarketRefreshLog(payload);
    if (message) log(message, String(payload.tone || "info"));
  });
}

export function formatRustMarketRefreshLog(event: Record<string, unknown>): string {
  const stage = String(event.stage || "");
  const data = asRecord(event.payload);
  if (stage === "command_received") return `Rust 命令已进入：种子 ${data.seed_stock_count || 0} 只，候选上限 ${data.max_candidates || "-"}，批次 ${Number(data.batch_start || 0) + 1} 起。`;
  if (stage === "candidate_ready") return `Rust 候选池已生成：${data.candidate_count || 0} 只，本地种子 ${data.seed_candidate_count || 0} 只。`;
  if (stage === "batch_window") return `Rust 批次窗口：${Number(data.batch_start || 0) + 1}-${data.batch_end || "?"}/${data.total_batches || "?"}，单批超时 ${data.batch_timeout_seconds || "?"} 秒。`;
  if (stage === "batch_response") return `Rust 批次 ${data.batch_index || "?"} 已返回：HTTP ${data.status || "-"}，${data.byte_len || 0} B，解析 ${data.parsed_count || 0} 只，耗时 ${data.elapsed_ms || "?"} ms。`;
  if (stage === "batch_error") return `Rust 批次 ${data.batch_index || "?"} 失败：${data.error || "unknown"}。`;
  if (stage === "command_complete") return `Rust 刷新完成：新增 ${data.fetched || 0} 只，保留 ${data.preserved || 0} 只，下一批 ${data.next_batch_start || 0}/${data.total_batches || "?"}。`;
  return stage ? `Rust 阶段 ${stage}：${JSON.stringify(data)}` : "";
}

export function isLikelyTradingDay(now = new Date()): boolean {
  const day = now.getDay();
  return day !== 0 && day !== 6;
}

export function shouldUsePreviousCloseForMobileRefresh(now = new Date()): boolean {
  if (!isLikelyTradingDay(now)) return false;
  return now.getHours() * 60 + now.getMinutes() < 15 * 60 + 5;
}

export function isMarketStatusStale(status: unknown): boolean {
  const data = asRecord(status);
  if (asRecord(data.policy).mode === "empty") return true;
  if (Object.prototype.hasOwnProperty.call(data, "universe_count") && Number(data.universe_count || 0) <= 0) return true;
  if (data.stale !== undefined) return Boolean(data.stale);
  const quoteDate = localTradeDateCompact(data.quote_generated_at || data.generated_at || data.universe_updated_at);
  return Boolean(quoteDate && quoteDate !== expectedMarketQuoteDateCompact());
}

export function mobileRefreshNotes(status: unknown, fallback = ""): string[] {
  const data = asRecord(status);
  const refresh = asRecord(data.refresh_result || data);
  const fetched = Number(refresh.fetched || 0);
  const requested = Number(refresh.requested || refresh.total_candidates || 0);
  const preserved = Number(refresh.preserved || 0);
  const failed = Number(refresh.failed_batches || 0);
  const totalBatches = Number(refresh.total_batches || 0);
  const completedBatches = Number(refresh.completed_batches || refresh.next_batch_start || 0);
  if (!Object.keys(refresh).length) return fallback ? [fallback] : [];
  const batchText = totalBatches > 0 ? `批次 ${Math.min(completedBatches, totalBatches)}/${totalBatches}` : "批次信息暂不可用";
  const base = fetched > 0
    ? `已联网更新 ${fetched} 只股票行情，保留 ${preserved} 只本地股票，候选 ${requested} 只，${batchText}。`
    : `行情暂未返回有效股票，已保留 ${preserved || Number(data.universe_count || 0)} 只本地股票，${batchText}。`;
  if (failed > 0) return [`${base} 失败批次 ${failed} 批，其余股票已继续处理。`];
  return [base];
}

export const MOBILE_TENCENT_MAX_CANDIDATES = 15000;
export const MOBILE_TENCENT_BATCHES_PER_STEP = 12;
export const OBSERVE_FULL_HISTORY_LIMIT = 10000;
export const OBSERVE_HISTORY_LIMIT = OBSERVE_FULL_HISTORY_LIMIT;

