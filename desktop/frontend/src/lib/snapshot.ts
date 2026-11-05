// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { normalizeStockCode, parseLooseNumber } from "./format";
import { formatLocalDataError } from "./localData";
import { normalizeMobileFinancialSnapshotDetails } from "./mobileFinancialSnapshot";
import { asRecord } from "./net";
import { jsonByteLength, logPerfRecord, objectRecordCount, perfDiagnosticsAreEnabled, perfOutcomeForError } from "./perfDiagnostics";
import { FINANCIAL_RATE_UNITS, METRIC_CONTRACT_VERSION, RateUnit, STOCK_RATE_UNITS, convertRate, financialSnapshotUnits } from "./rateUnits";
import { InvokeFn, isMobileTauriRuntime, isTauriRuntime } from "./tauriRuntime";

export const FRONTEND_BASE_URL =
  typeof window === "undefined" || !window.location?.href ? "http://localhost/" : window.location.href;
// 构建时注入的内容哈希（vite.config.ts 的 define）：文件名带哈希才能安全长缓存。
declare const __SNAPSHOT_FINANCIAL__: string | undefined;
declare const __SNAPSHOT_INDUSTRY__: string | undefined;
export function snapshotFileName(stem: string, version: string | undefined): string {
  return version ? `${stem}.${version}.json` : `${stem}.json`;
}
export const SNAPSHOT_CACHE: RequestCache = "force-cache";
export const BUNDLED_FINANCIAL_SNAPSHOT_URL = new URL(
  snapshotFileName("mobile-financial-snapshot", typeof __SNAPSHOT_FINANCIAL__ === "string" ? __SNAPSHOT_FINANCIAL__ : ""),
  FRONTEND_BASE_URL,
).toString();
export const BUNDLED_INDUSTRY_SNAPSHOT_URL = new URL(
  snapshotFileName("mobile-industry-snapshot", typeof __SNAPSHOT_INDUSTRY__ === "string" ? __SNAPSHOT_INDUSTRY__ : ""),
  FRONTEND_BASE_URL,
).toString();
export const DEDUCTED_FINANCIAL_FIELDS = [
  "deducted_net_profit_billion",
  "deducted_net_profit_margin",
  "deducted_net_profit_growth_rate",
];

export let financialSnapshotPromise: Promise<Record<string, unknown> | null> | null = null;
export let industrySnapshotPromise: Promise<Record<string, unknown> | null> | null = null;
export async function withFinancialSnapshotPayload(payload: unknown): Promise<unknown> {
  if (!isTauriRuntime()) return payload;
  const base = asRecord(payload);
  const requestedIndustry = typeof asRecord(base.criteria).industry === "string"
    ? String(asRecord(base.criteria).industry).trim()
    : "";
  const existingSnapshot = asRecord(base.financial_snapshot);
  const existingPayload = financialSnapshotPayload(existingSnapshot);
  if (existingPayload && (!requestedIndustry || Object.keys(asRecord(existingPayload.industries)).length)) {
    return { ...base, financial_snapshot: existingPayload };
  }
  const snapshot = await loadBundledScreenSnapshot();
  const existingStocks = Array.isArray(existingSnapshot.stocks) ? existingSnapshot.stocks : [];
  const financialSnapshot = financialSnapshotPayload({
    schema_version: "mobile-financial-snapshot/v4",
    metric_units: FINANCIAL_RATE_UNITS,
    stock_metric_units: STOCK_RATE_UNITS,
    stocks: Array.isArray(existingPayload?.stocks) ? existingPayload.stocks : existingStocks,
    financials: {
      ...asRecord(snapshot?.financials),
      ...asRecord(existingPayload?.financials),
    },
    industries: {
      ...asRecord(snapshot?.industries),
      ...asRecord(existingSnapshot.industries),
    },
  }, { includeStocks: existingStocks.length > 0 });
  if (requestedIndustry && !Object.keys(asRecord(financialSnapshot?.industries)).length) {
    throw new Error("行业分类数据加载失败，无法执行行业筛选，请重试。");
  }
  return financialSnapshot ? { ...base, financial_snapshot: financialSnapshot } : payload;
}

export async function invokeLocalData(invoke: InvokeFn, command: string, payload: unknown): Promise<unknown> {
  try { return await invoke(command, { payload }); }
  catch (error) { throw new Error(formatLocalDataError(error), { cause: error }); }
}
export async function invokeWithFinancialSnapshot(invoke: InvokeFn, command: string, payload: unknown): Promise<unknown> {
  const base: Record<string, unknown> = { data_policy: "cache_only", ...asRecord(payload) };
  const mobileRuntime = isMobileTauriRuntime();
  const perfEnabled = command === "api_screen" && perfDiagnosticsAreEnabled(mobileRuntime);
  if (perfEnabled) base.perf_diagnostics = true;
  const snapshotStartedAt = perfEnabled ? performance.now() : 0;
  let nextPayload: unknown = base;
  if (base.data_policy === "refresh") {
    try {
      nextPayload = await withFinancialSnapshotPayload(base);
    } catch (error) {
      if (command === "api_screen") {
        logPerfRecord({
          flow: "screening",
          stage: "financial_snapshot_prefetch",
          elapsedMs: performance.now() - snapshotStartedAt,
          cacheStrategy: "refresh",
          outcome: perfOutcomeForError(error),
        }, mobileRuntime);
      }
      throw error;
    }
  }
  const nextPayloadRecord = asRecord(nextPayload);
  if (command === "api_screen" && base.data_policy === "refresh") {
    const snapshot = asRecord(nextPayloadRecord.financial_snapshot);
    logPerfRecord({
      flow: "screening",
      stage: "financial_snapshot_prefetch",
      elapsedMs: performance.now() - snapshotStartedAt,
      cacheStrategy: "refresh",
      stockCount: Array.isArray(snapshot.stocks) ? snapshot.stocks.length : undefined,
      financialRecords: objectRecordCount(snapshot.financials),
      outcome: "ok",
    }, mobileRuntime);
  }
  const requestBytes = perfEnabled ? jsonByteLength(nextPayloadRecord) : undefined;
  const invokeStartedAt = perfEnabled ? performance.now() : 0;
  try {
    const result = await invokeLocalData(invoke, command, nextPayloadRecord);
    if (command === "api_screen") {
      const screenResult = asRecord(result);
      logPerfRecord({
        flow: "screening",
        stage: "ipc_roundtrip",
        elapsedMs: perfEnabled ? performance.now() - invokeStartedAt : 0,
        cacheStrategy: base.data_policy === "refresh" ? "refresh" : "cache_only",
        stockCount: Array.isArray(screenResult.results)
          ? screenResult.results.length
          : Array.isArray(screenResult.stocks)
            ? screenResult.stocks.length
            : undefined,
        requestBytes,
        responseBytes: perfEnabled ? jsonByteLength(result) : undefined,
        outcome: "ok",
      }, mobileRuntime);
    }
    return result;
  } catch (error) {
    if (command === "api_screen") {
      logPerfRecord({
        flow: "screening",
        stage: "ipc_roundtrip",
        elapsedMs: perfEnabled ? performance.now() - invokeStartedAt : 0,
        cacheStrategy: base.data_policy === "refresh" ? "refresh" : "cache_only",
        requestBytes,
        outcome: perfOutcomeForError(error),
      }, mobileRuntime);
    }
    throw error;
  }
}

export async function invokeBacktest(invoke: InvokeFn, payload: unknown): Promise<unknown> {
  const base = { data_policy: "refresh", ...asRecord(payload) };
  return invokeLocalData(invoke, "api_backtest", base);
}

export async function loadBundledFinancialSnapshot(): Promise<Record<string, unknown> | null> {
  if (financialSnapshotPromise) return financialSnapshotPromise;
  financialSnapshotPromise = (async () => {
    try {
      const response = await fetch(BUNDLED_FINANCIAL_SNAPSHOT_URL, { cache: SNAPSHOT_CACHE });
      if (!response.ok) return null;
      const snapshot = asRecord(await response.json());
      const units = financialSnapshotUnits(snapshot, true);
      const stockUnits = stockSnapshotInputUnits(snapshot, units);
      const financials: Record<string, unknown> = {};
      const stocks = (Array.isArray(snapshot.stocks) ? snapshot.stocks : [])
        .map((stock) => normalizeBundledFinancialSnapshotStock(asRecord(stock), stockUnits, units))
        .filter(Boolean) as Record<string, unknown>[];
      for (const stock of stocks) {
        const code = String(stock.code || "");
        const financial = asRecord(stock.financial);
        if (code && Object.keys(financial).length) financials[code] = financial;
      }
      for (const [rawCode, value] of Object.entries(asRecord(snapshot.financials))) {
        const code = normalizeStockCode(rawCode);
        const financial = normalizeBundledFinancialSnapshotFinancial(asRecord(value), units);
        if (code && financial) financials[code] = financial;
      }
      const industries: Record<string, string> = {};
      for (const [rawCode, value] of Object.entries(asRecord(snapshot.industries))) {
        const code = normalizeStockCode(rawCode);
        const industry = typeof value === "string" ? value.trim() : "";
        if (code && industry && industry !== "-") industries[code] = industry;
      }
      return {
        ...snapshot, schema_version: "mobile-financial-snapshot/v4",
        metric_contract_version: METRIC_CONTRACT_VERSION, metric_units: FINANCIAL_RATE_UNITS,
        stock_metric_units: STOCK_RATE_UNITS,
        stocks, financials, industries,
      };
    } catch (error) {
      if (error instanceof Error && error.message.includes("单位")) throw error;
      return null;
    }
  })();
  return financialSnapshotPromise;
}

export async function loadBundledIndustrySnapshot(): Promise<Record<string, unknown> | null> {
  if (industrySnapshotPromise) return industrySnapshotPromise;
  const loadPromise = (async () => {
    try {
      const response = await fetch(BUNDLED_INDUSTRY_SNAPSHOT_URL, { cache: SNAPSHOT_CACHE });
      if (!response.ok) return null;
      const snapshot = asRecord(await response.json());
      const industries: Record<string, string> = {};
      for (const [rawCode, value] of Object.entries(asRecord(snapshot.industries))) {
        const code = normalizeStockCode(rawCode);
        const industry = typeof value === "string" ? value.trim() : "";
        if (code && industry && industry !== "-") industries[code] = industry;
      }
      return Object.keys(industries).length ? { ...snapshot, industries } : null;
    } catch {
      return null;
    }
  })();
  industrySnapshotPromise = loadPromise.then((snapshot) => {
    if (!snapshot) industrySnapshotPromise = null;
    return snapshot;
  });
  return industrySnapshotPromise;
}

export async function loadBundledScreenSnapshot(): Promise<Record<string, unknown> | null> {
  const [financialSnapshot, industrySnapshot] = await Promise.all([
    loadBundledFinancialSnapshot(),
    loadBundledIndustrySnapshot(),
  ]);
  if (!financialSnapshot && !industrySnapshot) return null;
  return {
    ...(financialSnapshot || {}),
    industries: {
      ...asRecord(financialSnapshot?.industries),
      ...asRecord(industrySnapshot?.industries),
    },
  };
}

export function financialSnapshotPayload(
  snapshot: Record<string, unknown> | null,
  options: { includeStocks?: boolean } = {},
): Record<string, unknown> | null {
  if (!snapshot) return null;
  const includeStocks = options.includeStocks !== false;
  const sourceStocks = includeStocks && Array.isArray(snapshot.stocks) ? snapshot.stocks : [];
  const sourceFinancials = asRecord(snapshot.financials);
  const industries = asRecord(snapshot.industries);
  if (!sourceStocks.length && !Object.keys(sourceFinancials).length && !Object.keys(industries).length) return null;
  const hasRates = [...sourceStocks, ...Object.values(sourceFinancials)].some((value) => {
    const row = asRecord(value);
    const nested = asRecord(row.financial);
    return Object.keys(FINANCIAL_RATE_UNITS).some((field) => parseLooseNumber(row[field]) !== null || parseLooseNumber(nested[field]) !== null);
  });
  const units = hasRates ? financialSnapshotUnits(snapshot) : FINANCIAL_RATE_UNITS;
  const stockUnits = stockSnapshotInputUnits(snapshot, units);
  const stocks = sourceStocks.map((stock) => normalizeBundledFinancialSnapshotStock(asRecord(stock), stockUnits, units))
    .filter((stock): stock is Record<string, unknown> => stock !== null);
  const financials: Record<string, unknown> = {};
  for (const stock of stocks) {
    const financial = asRecord(stock.financial);
    if (Object.keys(financial).length) financials[String(stock.code)] = financial;
  }
  for (const [code, raw] of Object.entries(sourceFinancials)) {
    const normalized = normalizeBundledFinancialSnapshotFinancial(asRecord(raw), units);
    if (normalized) financials[code] = normalized;
  }
  const metadata = {
    schema_version: "mobile-financial-snapshot/v4",
    metric_contract_version: METRIC_CONTRACT_VERSION,
    metric_units: FINANCIAL_RATE_UNITS,
    stock_metric_units: STOCK_RATE_UNITS,
  };
  return includeStocks ? { ...metadata, stocks, financials, industries } : { ...metadata, financials, industries };
}

export async function loadBundledFinancialSnapshotForCode(rawCode: string): Promise<Record<string, unknown> | null> {
  const code = normalizeStockCode(rawCode);
  if (!code) return null;
  const snapshot = await loadBundledFinancialSnapshot();
  const financial = asRecord(asRecord(snapshot?.financials)[code]);
  if (!Object.keys(financial).length) return null;
  return {
    schema_version: "mobile-financial-snapshot/v4", metric_contract_version: METRIC_CONTRACT_VERSION,
    metric_units: FINANCIAL_RATE_UNITS, stocks: [], financials: { [code]: financial },
  };
}

export function normalizeBundledFinancialSnapshotStock(
  stock: Record<string, unknown>, units: Readonly<Record<string, RateUnit>>,
  financialUnits: Readonly<Record<string, RateUnit>>,
): Record<string, unknown> | null {
  const code = normalizeStockCode(stock.code);
  if (!code) return null;
  const item: Record<string, unknown> = { ...stock, code };
  const nested = asRecord(stock.financial);
  const financial = Object.keys(nested).length
    ? normalizeBundledFinancialSnapshotFinancial(nested, financialUnits)
    : normalizeBundledFinancialSnapshotFinancial(stock, units);
  for (const field of DEDUCTED_FINANCIAL_FIELDS) {
    const value = parseLooseNumber(financial?.[field]);
    if (value !== null) item[field] = value;
  }
  for (const [field, target] of Object.entries(STOCK_RATE_UNITS)) {
    if (parseLooseNumber(stock[field]) === null) continue;
    const sourceUnit = units[field];
    if (!sourceUnit) throw new Error(`股票指标 ${field} 缺少明确单位`);
    const converted = convertRate(stock[field], sourceUnit, target);
    if (converted !== null) item[field] = converted;
  }
  if (financial) item.financial = financial;
  return item;
}

export function stockSnapshotInputUnits(
  snapshot: Record<string, unknown>, financialUnits: Readonly<Record<string, RateUnit>>,
): Readonly<Record<string, RateUnit>> {
  const units: Record<string, RateUnit> = { change_pct: "ratio", turnover_rate: "ratio", ...financialUnits };
  if (snapshot.stock_metric_units === undefined) return units;
  const declared = snapshot.stock_metric_units;
  if (!declared || typeof declared !== "object" || Array.isArray(declared)) throw new Error("股票快照单位声明无效");
  for (const [field, unit] of Object.entries(declared)) {
    if ((!Object.hasOwn(STOCK_RATE_UNITS, field) && !Object.hasOwn(FINANCIAL_RATE_UNITS, field)) || (unit !== "ratio" && unit !== "percentage_points")) {
      throw new Error(`股票指标 ${field} 的单位无效`);
    }
    units[field] = unit;
  }
  return units;
}

export function normalizeBundledFinancialSnapshotFinancial(
  source: Record<string, unknown>, units: Readonly<Record<string, RateUnit>>,
): Record<string, unknown> | null {
  const financial: Record<string, unknown> = normalizeMobileFinancialSnapshotDetails(source, units);
  const profit = parseLooseNumber(source.deducted_net_profit_billion);
  if (profit !== null) financial.deducted_net_profit_billion = profit;
  const latestEps = parseLooseNumber(source.latest_eps ?? source.eps);
  if (latestEps !== null) financial.latest_eps = latestEps;
  const latestBps = parseLooseNumber(source.latest_bps ?? source.bps);
  if (latestBps !== null) financial.latest_bps = latestBps;
  const period = normalizeFinancialPeriodKey(source.period || source.latest_period || source.latest_eps_period);
  if (period) financial.period = period;
  if (source.source) financial.source = String(source.source);
  const quarterly = (Array.isArray(source.quarterly_eps) ? source.quarterly_eps : [])
    .map((point) => normalizeQuarterlyEpsPoint(asRecord(point)))
    .filter(Boolean) as Record<string, unknown>[];
  if (quarterly.length) {
    financial.quarterly_eps = quarterly;
    if (!financial.period) financial.period = quarterly[0].period;
  }
  return Object.keys(financial).length ? financial : null;
}

export function normalizeQuarterlyEpsPoint(point: Record<string, unknown>): Record<string, unknown> | null {
  const period = normalizeFinancialPeriodKey(point.period);
  const value = parseLooseNumber(point.value ?? point.raw_value);
  if (!period || value === null) return null;
  return { period, value, source: point.source ? String(point.source) : "bundled financial snapshot" };
}

export function normalizeFinancialPeriodKey(value: unknown): string {
  const raw = String(value || "").trim();
  const quarter = raw.match(/(20\d{2})\s*Q\s*([1-4])/i);
  if (quarter) return `${quarter[1]}Q${quarter[2]}`;
  const date = raw.match(/(20\d{2})[-/.年\s]*(0?[1-9]|1[0-2])/);
  if (!date) return "";
  const month = Number(date[2]);
  const q = month <= 3 ? 1 : month <= 6 ? 2 : month <= 9 ? 3 : 4;
  return `${date[1]}Q${q}`;
}
