// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { currentSystemDateCompact, normalizeDateParam, normalizeStockCode, parseLooseNumber } from "./format";
import { asRecord, fetchWithTimeout } from "./net";
import { fetchObserveDailyHistoryRows } from "./observeHistory";
import { OBSERVE_HISTORY_LIMIT } from "./marketRefresh";

export async function fetchObserveDailyHistoryForTauri(payload: Record<string, unknown>, timeoutMs: number): Promise<Record<string, unknown>[] | null> {
  const symbol = observeTencentDailySymbol(String(payload.code || ""));
  if (!symbol) return null;
  const start = normalizeDateParam(payload.start_date, "20200101");
  const end = normalizeDateParam(payload.end_date, currentSystemDateCompact());
  const param = `${symbol},day,${start},${end},${OBSERVE_HISTORY_LIMIT},`;
  const url = `https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=${encodeURIComponent(param)}`;
  const response = await fetchWithTimeout(url, { cache: "no-store" }, timeoutMs, `WebView Tencent 日线超过 ${Math.round(timeoutMs / 1000)} 秒未返回。`);
  if (!response.ok) throw new Error(`WebView Tencent 日线 HTTP ${response.status}`);
  const data = asRecord(await response.json());
  const quote = asRecord(asRecord(data.data)[symbol]);
  const rows = quote.day || quote.qfqday || [];
  return (Array.isArray(rows) ? rows : []).map(observeTencentDailyRowToHistory).filter(Boolean) as Record<string, unknown>[];
}

export function observeTencentDailySymbol(rawCode: string): string {
  const code = normalizeStockCode(rawCode);
  if (!code) return "";
  const [digits, market] = code.split(".");
  return `${String(market || "").toLowerCase()}${digits}`;
}

export function observeTencentDailyRowToHistory(row: unknown): Record<string, unknown> | null {
  if (!Array.isArray(row) || row.length < 6) return null;
  const close = parseLooseNumber(row[2]);
  if (close === null) return null;
  return {
    date: String(row[0] || ""),
    open: parseLooseNumber(row[1]) ?? close,
    close,
    high: parseLooseNumber(row[3]) ?? close,
    low: parseLooseNumber(row[4]) ?? close,
    volume: parseLooseNumber(row[5]),
  };
}

export async function fetchObserveDailyHistoryForTauriStable(payload: Record<string, unknown>, timeoutMs: number): Promise<Record<string, unknown>[] | null> {
  return fetchObserveDailyHistoryRows(payload, timeoutMs, fetchWithTimeout);
}
