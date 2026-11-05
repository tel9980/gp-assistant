// 从 ObservePanel.tsx 拆出的纯逻辑/子组件；由原文件按需 import。
import { currentSystemDateInputValue, formatNumber, formatPercent, formatPrice, formatRatioPercent, kdjRangeLabel } from "../../lib/format";
import { isFinancialPeriodStale } from "../../lib/fundamentalSnapshot";
import { computeKdj, toDailyBars } from "../../lib/kline";
import { buildMainFundFlowView, hasMainFundFlowBreakdown } from "../../lib/mainFundFlow";
import { CapitalEvidenceItem, CapitalEvidenceResult, CapitalEvidenceSection, FinancialIndicatorItem, ObserveResult, StockItem, TrendIndicatorPoint, TrendIndicatorSignal } from "../../types/index";
export function observeAgentPrompt(name: string, code: string): string {
  const label = name.trim() || code;
  return `请研究 ${label}（${code}）。结合行情、财务和趋势，说明现在值得继续观察还是需要回避，并指出主要依据。`;
}

export function needsFinancialSnapshotRefresh(result: ObserveResult): boolean {
  return isFinancialPeriodStale(result.financial_indicators?.period, currentSystemDateInputValue());
}

export function needsPublicMarketDataRefresh(result: ObserveResult): boolean {
  const checkedOn = currentSystemDateInputValue();
  const active = result.market_active_brokerages;
  const holdings = result.institutional_holdings;
  return !active
    || active.checked_on !== checkedOn
    || active.status !== "available"
    || !holdings
    || holdings.checked_on !== checkedOn
    || holdings.status === "unavailable";
}

export function needsMainFundFlowRefresh(result: ObserveResult): boolean {
  const flow = buildMainFundFlowView(result.capital_evidence);
  return flow.available && !hasMainFundFlowBreakdown(result.capital_evidence);
}

export const OBSERVE_FULL_HISTORY_START = "1990-01-01";
export const OBSERVE_FULL_HISTORY_LIMIT = "10000";
export function capitalMetricValue(item: CapitalEvidenceItem | null | undefined, key: string): string {
  const value = item?.metrics?.[key];
  if (value == null) return "暂无";
  const text = String(value).trim();
  return !text || text === "-" || text === "--" || text === "—" ? "暂无" : text;
}

export function capitalMetricPart(label: string, value: string): string {
  return value === "暂无" ? "" : `${label} ${value}`;
}

export function isMissingMetricText(value: string): boolean {
  const text = String(value || "").trim();
  return !text || text === "暂无" || text === "--" || text === "—" || text === "-";
}

export function proxyDirectionFromScore(score: number): string {
  if (!Number.isFinite(score)) return "暂无";
  if (score >= 60) return "偏流入";
  if (score <= 40) return "偏流出";
  return "中性";
}

export type MetricAnalysisTone = "positive" | "warning" | "negative" | "neutral";

export type MetricAnalysisItem = {
  title: string;
  text: string;
  tone: MetricAnalysisTone;
};

export function selectObservationDecisions(items: MetricAnalysisItem[]): MetricAnalysisItem[] {
  const preferred = ["趋势位置", "行情估值", "财务资金"]
    .map((title) => items.find((item) => item.title === title))
    .filter((item): item is MetricAnalysisItem => Boolean(item));
  return preferred.length ? preferred : items.slice(0, 3);
}

export function firstSentence(text: string): string {
  const sentence = String(text || "").split("。").find((item) => item.trim());
  return sentence ? `${sentence.trim()}。` : "当前证据不足。";
}

export function observationDecisionLabel(title: string): string {
  const labels: Record<string, string> = {
    趋势位置: "趋势位置",
    行情估值: "价格表现",
    财务资金: "财务概览",
  };
  return labels[title] || title;
}

export function buildObservationVerdict(signal?: TrendIndicatorSignal | null): { title: string; summary: string; tone: MetricAnalysisTone } {
  const tone = signalStatusTone(signal?.status);
  const risks = Array.isArray(signal?.risk_flags) ? signal.risk_flags.map(riskFlagLabel).filter(Boolean) : [];
  if (risks.length) {
    return {
      title: tone === "negative" ? "趋势承压，先等待风险收敛" : "已有方向，但风险尚未解除",
      summary: `当前识别到${risks.slice(0, 3).join("、")}。先确认风险信号是否消退，再判断趋势是否成立。`,
      tone: tone === "negative" ? "negative" : "warning",
    };
  }
  if (tone === "positive") return { title: "趋势偏强，进入确认阶段", summary: "积极结构已经出现，下一步验证压力位突破和量能延续。", tone };
  if (tone === "negative") return { title: "趋势偏弱，暂不急于下结论", summary: "价格与趋势结构仍承压，重点观察支撑是否有效。", tone };
  if (tone === "warning") return { title: "信号未确认，适合继续观察", summary: "当前有变化，但证据还不足以形成稳定方向。", tone };
  return { title: "方向尚未形成，等待关键信号", summary: "当前证据偏中性，优先观察趋势、量能和关键价位能否形成一致。", tone: "neutral" };
}

export function buildObservationNextSignal(signal?: TrendIndicatorSignal | null): string {
  const support = formatPrice(signal?.support);
  const resistance = formatPrice(signal?.resistance);
  const supportText = support !== "--" ? `${support} 支撑` : "关键支撑";
  const resistanceText = resistance !== "--" ? `${resistance} 压力位` : "上方压力位";
  const tone = signalStatusTone(signal?.status);
  if (signal?.kdj_overbought) return `先看超买状态能否降温，同时确认回落时 ${supportText} 是否有效。`;
  if (signal?.kdj_dead_cross) return `先看 KDJ 死叉是否修复，并确认价格能否守住 ${supportText}。`;
  if (tone === "positive") return `关注能否放量突破 ${resistanceText}；若回落，再看 ${supportText} 是否有效。`;
  if (tone === "negative") return `关注价格能否重新站稳 ${supportText}，并等待短期均线与长期均线关系改善。`;
  return `等待趋势方向与成交量形成一致；向上关注 ${resistanceText}，向下关注 ${supportText}。`;
}
export function buildMetricAnalysis({
  stock,
  signal,
  latest,
  kdj,
  changePct,
  capital,
  capitalSummary,
  financialLookup,
}: {
  stock: StockItem;
  signal?: TrendIndicatorSignal | null;
  latest: TrendIndicatorPoint | null;
  kdj: { k?: number | null; d?: number | null; j?: number | null } | null;
  changePct: unknown;
  capital?: CapitalEvidenceResult | null;
  capitalSummary: Record<string, string>;
  financialLookup: Map<string, string>;
}): MetricAnalysisItem[] {
  const items: MetricAnalysisItem[] = [];
  const pct = numeric(changePct);
  const pe = numeric(stock.pe);
  const pb = numeric(stock.pb);
  const roe = numeric(stock.roe);
  const quant = numeric(signal?.quant_score);
  const quantMax = numeric(signal?.quant_score_max);
  const pattern = numeric(signal?.pattern_score);
  const patternMax = numeric(signal?.pattern_score_max);
  const capitalScore = numeric(capital?.composite_score);
  const accumulation = numeric(latest?.accumulation_strength ?? latest?.accumulation_index);
  const trendHeat = numeric(latest?.trend_heat);
  const volumeHeat = numeric(latest?.volume_price_heat);
  const k = numeric(kdj?.k);
  const d = numeric(kdj?.d);
  const j = numeric(kdj?.j);

  const priceText = Number.isFinite(pct)
    ? `近端涨跌幅为 ${formatSignedPercent(pct)}，${pct > 0.03 ? "短线情绪偏强" : pct < -0.03 ? "短线承压较明显" : pct > 0 ? "价格小幅走强" : pct < 0 ? "价格小幅回落" : "价格变化不大"}。`
    : "当前缺少可用涨跌幅，价格动量需要结合图表确认。";
  const valuationBits = [
    Number.isFinite(pe) ? `PE ${formatNumber(pe)}` : "PE 暂缺",
    Number.isFinite(pb) ? `PB ${formatNumber(pb)}` : "PB 暂缺",
    Number.isFinite(roe) ? `ROE ${formatRatioPercent(roe)}` : "ROE 暂缺",
  ];
  items.push({
    title: "行情估值",
    tone: pct > 0.03 ? "positive" : pct < -0.03 ? "negative" : "neutral",
    text: `${priceText} 估值侧 ${valuationBits.join("、")}，更适合与同行业分位做横向比较。`,
  });

  const signalState = signalStatusLabel(signal?.status);
  const signalType = signalTypeLabel(signal?.signal_type);
  const trendPosition = signal?.swl_above_sws === true ? "短期均线位于长期均线上方，趋势结构偏积极" : signal?.swl_above_sws === false ? "短期均线位于长期均线下方，趋势结构仍需观察" : "短期均线/长期均线关系暂不完整";
  const riskText = Array.isArray(signal?.risk_flags) && signal.risk_flags.length ? `风险标记包括 ${formatList(signal.risk_flags, riskFlagLabel)}。` : "暂未识别到明确风险标记。";
  items.push({
    title: "趋势位置",
    tone: signalStatusTone(signal?.status),
    text: `当前信号为${signalState}，类型为${signalType}；${trendPosition}。${riskText}`,
  });

  if (Number.isFinite(k) || Number.isFinite(d) || Number.isFinite(j) || signal?.kdj_golden_cross || signal?.kdj_dead_cross) {
    const cross = signal?.kdj_golden_cross ? "出现金叉信号" : signal?.kdj_dead_cross ? "出现死叉信号" : "暂未出现明显交叉信号";
    const range = kdjRangeLabel(signal || {});
    const zone = range === "高低位信号分歧" ? "高低位信号分歧，需要结合各线数值和方向核验"
      : range === "超买" ? "处于超买区，追高风险上升" : range === "超卖" ? "处于超卖区，反弹信号需要成交确认" : range;
    items.push({
      title: "KDJ 信号",
      tone: signal?.kdj_dead_cross || range === "超买" || range === "高低位信号分歧" ? "warning" : signal?.kdj_golden_cross ? "positive" : "neutral",
      text: `K/D/J 为 ${formatNumber(k)} / ${formatNumber(d)} / ${formatNumber(j)}，${cross}，${zone}。`,
    });
  }

  if (Number.isFinite(accumulation) || Number.isFinite(trendHeat) || Number.isFinite(volumeHeat)) {
    const heatText = [
      Number.isFinite(accumulation) ? `吸筹强度 ${formatNumber(accumulation)}` : "吸筹暂缺",
      Number.isFinite(trendHeat) ? `趋势热度 ${formatNumber(trendHeat)}` : "趋势热度暂缺",
      Number.isFinite(volumeHeat) ? `量价热度 ${formatNumber(volumeHeat)}` : "量价热度暂缺",
    ].join("、");
    const tone: MetricAnalysisTone = accumulation >= 60 || trendHeat >= 60 || volumeHeat >= 60 ? "positive" : accumulation <= 20 && trendHeat <= 20 ? "warning" : "neutral";
    items.push({
      title: "热度吸筹",
      tone,
      text: `${heatText}。若热度走强但价格接近压力位，应重点观察量能延续性。`,
    });
  }

  if (Number.isFinite(quant) || Number.isFinite(pattern)) {
    const quantRatio = Number.isFinite(quant) && Number.isFinite(quantMax) && quantMax > 0 ? quant / quantMax : NaN;
    const patternRatio = Number.isFinite(pattern) && Number.isFinite(patternMax) && patternMax > 0 ? pattern / patternMax : NaN;
    const scoreTone: MetricAnalysisTone = quantRatio >= 0.65 || patternRatio >= 0.65 ? "positive" : quantRatio <= 0.35 && patternRatio <= 0.35 ? "warning" : "neutral";
    items.push({
      title: "模型评分",
      tone: scoreTone,
      text: `量化分 ${formatScore(signal?.quant_score, signal?.quant_score_max)}，形态分 ${formatScore(signal?.pattern_score, signal?.pattern_score_max)}；评分用于排序和观察，不等同于确定性结论。`,
    });
  }

  const eps = financialMetricValue(financialLookup, [
    "latest_eps",
    "eps",
    "EPS",
    "最新每股收益",
    "每股收益",
    "estimated_eps",
    "每股收益(估算)",
    "每股收益(计算)",
  ]) || formatStockEps(stock);
  const profitGrowth = financialMetricValue(financialLookup, [
    "deducted_net_profit_growth_rate",
    "扣非净利润增长率",
    "扣非净利同比",
    "扣非增长",
  ]) || formatStockProfitGrowth(stock);
  const capitalText = capital ? `资金综合分 ${formatNumber(capitalScore)}，证据置信度 ${capital.confidence || "--"}；资金流 ${capitalSummary.fundFlow}。` : "资金证据暂缺，不能把缺失数据解读为流入或流出。";
  items.push({
    title: "财务资金",
    tone: capitalScore >= 60 ? "positive" : capitalScore <= 30 ? "warning" : "neutral",
    text: `财务侧 EPS ${eps || "暂缺"}、扣非增长 ${profitGrowth || "暂缺"}。${capitalText}`,
  });

  return items;
}
export function buildFinancialLookup(items: FinancialIndicatorItem[]): Map<string, string> {
  const lookup = new Map<string, string>();
  for (const item of items || []) {
    const value = item.value == null || item.value === "" ? "" : String(item.value);
    if (!value) continue;
    if (item.metric_key) lookup.set(item.metric_key, value);
    if (item.label) lookup.set(item.label, value);
  }
  return lookup;
}

export function financialMetricValue(lookup: Map<string, string>, keys: string[]): string {
  for (const key of keys) {
    const value = lookup.get(key)?.trim();
    if (value && !["--", "—", "暂无", "暂缺"].includes(value)) return value;
  }
  return "";
}

export function formatStockEps(stock: StockItem): string {
  const value = formatNumber(stock.latest_eps ?? stock.eps);
  return value === "—" ? "" : `${value}元`;
}

export function formatStockProfitGrowth(stock: StockItem): string {
  const value = formatPercent(stock.deducted_net_profit_growth_rate);
  return value === "—" ? "" : value;
}
export function summarizeCapitalEvidence(capital?: CapitalEvidenceResult | null): Record<string, string> {
  const sections = capital?.sections || [];
  return {
    fundFlow: evidenceSectionSummary(sections, ["fund_flow", "资金流"]),
    news: evidenceSectionSummary(sections, ["news_sentiment", "message_sentiment", "消息"]),
    technical: evidenceSectionSummary(sections, ["technical_behavior", "technical", "技术"]),
  };
}

export function evidenceSectionSummary(sections: CapitalEvidenceSection[], keys: string[]): string {
  const section = sections.find((item) => keys.some((key) => item.key?.includes(key) || item.title?.includes(key)));
  if (!section) return "--";
  const score = section.score == null ? "" : `${formatNumber(section.score)}分`;
  const state = section.available === false ? "暂无证据" : section.summary || section.items?.[0]?.title || "有证据";
  return [score, state].filter(Boolean).join(" · ");
}

export function compactMetricItems(items: Array<[string, string]>): Array<[string, string]> {
  return items.map(([label, value]) => [label, value || "--"]);
}

export function numeric(value: unknown): number {
  if (value == null || value === "") return NaN;
  const n = Number(value);
  return Number.isFinite(n) ? n : NaN;
}

export function ratioChange(value: unknown, base: unknown): number | null {
  const current = numeric(value);
  const previous = numeric(base);
  if (!Number.isFinite(current) || !Number.isFinite(previous) || previous === 0) return null;
  return (current - previous) / previous;
}

export function formatSignedNumber(value: unknown): string {
  const n = numeric(value);
  if (!Number.isFinite(n)) return "--";
  return `${n > 0 ? "+" : ""}${n.toFixed(2)}`;
}

export function formatSignedPercent(value: unknown): string {
  const n = numeric(value);
  if (!Number.isFinite(n)) return "--";
  const percent = Math.abs(n) <= 1 ? n * 100 : n;
  return `${percent > 0 ? "+" : ""}${percent.toFixed(2)}%`;
}

export function formatObserveTime(value: unknown, fallback = "数据时间未知"): string {
  if (value == null || value === "") return fallback;
  const text = String(value).trim();
  if (!text) return fallback;

  const compact = text.match(/^(\d{4})(\d{2})(\d{2})(?:(\d{2})(\d{2})(\d{2})?)?$/);
  if (compact) {
    const [, year, month, day, hour, minute] = compact;
    return hour && minute ? `${year}-${month}-${day} ${hour}:${minute}` : `${year}-${month}-${day}`;
  }

  const separated = text.match(/^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?:[ T](\d{1,2}):(\d{2})(?::\d{2})?)?/);
  if (separated) {
    const [, year, month, day, hour, minute] = separated;
    const date = `${year}-${month.padStart(2, "0")}-${day.padStart(2, "0")}`;
    return hour && minute ? `${date} ${hour.padStart(2, "0")}:${minute}` : date;
  }

  return text;
}

export function formatMarketCap(stock: StockItem): string {
  if (stock.market_cap_billion != null) return `${formatNumber(stock.market_cap_billion)}亿`;
  return formatNumber(stock.market_cap);
}


export function formatScore(value: unknown, max: unknown): string {
  const n = numeric(value);
  if (!Number.isFinite(n)) return "--";
  const m = numeric(max);
  return Number.isFinite(m) && m > 0 ? `${n}/${m}` : String(n);
}

export function formatList(values: unknown, mapper?: (value: unknown) => string): string {
  if (!Array.isArray(values) || !values.length) return "--";
  return values.map((item) => mapper ? mapper(item) : String(item)).filter(Boolean).join("、") || "--";
}

export const warnedObserveLabelValues = new Set<string>();

export function unmappedObserveLabel(kind: string, value: unknown, fallback = "未识别类型"): string {
  const text = String(value || "").trim();
  if (!text) return "--";
  const key = `${kind}:${text}`;
  if (!warnedObserveLabelValues.has(key)) {
    warnedObserveLabelValues.add(key);
    console.warn(`[observe-summary] unmapped ${kind}: ${text}`);
  }
  return fallback;
}

export function signalTypeLabel(value: unknown): string {
  const labels: Record<string, string> = {
    bullish: "看多",
    bearish: "看空",
    neutral: "中性",
    watch: "观察",
    buy_setup: "买点预备",
    exit: "离场",
    uptrend: "上升趋势",
    trend_continuation: "趋势延续",
    trend_reversal: "趋势反转",
    range_bound: "区间震荡",
    breakout_attempt: "突破尝试",
    risk_warning: "风险预警",
    breakout_chase: "突破追踪",
    pullback_buy: "回踩买点",
  };
  const key = String(value || "").trim();
  return labels[key] || unmappedObserveLabel("signal_type", value);
}

export function riskFlagLabel(value: unknown): string {
  const labels: Record<string, string> = {
    high_upper_shadow: "上影线偏长",
    bearish_long_ma_stack: "均线空头排列",
    kdj_overbought: "KDJ 超买",
    kdj_dead_cross: "KDJ 死叉",
    below_support: "跌破支撑",
    insufficient_history: "历史样本不足",
    low_volume: "成交量不足",
    breakdown_ma20: "跌破 20 日均线",
    white_exit: "白线离场",
    volume_stall: "放量滞涨",
    macd_bearish_divergence: "MACD 顶背离",
    swl_below_sws: "短期均线低于长期均线",
    weak_final_score: "综合评分偏弱",
  };
  const key = String(value || "").trim();
  return labels[key] || unmappedObserveLabel("risk_flag", value);
}

export function patternSignalLabel(value: unknown): string {
  const labels: Record<string, string> = {
    red_hold: "红持",
    cyan_watch: "青观",
    short_buy: "短买",
    white_exit: "白离",
    swl_above_sws: "短期均线高于长期均线",
    swl_below_sws: "短期均线低于长期均线",
    kdj_golden_cross: "KDJ 金叉",
    kdj_dead_cross: "KDJ 死叉",
    accumulation_strength: "吸筹增强",
    bottom_accumulation: "底部吸筹",
    swing_opportunity: "波段机会",
    rebound_signal: "反弹信号",
    dragon_trend_volume: "趋势量能共振",
  };
  const key = String(value || "").trim();
  return labels[key] || unmappedObserveLabel("pattern_signal", value);
}

export function kdjFromSignal(signal: { k?: number | null; d?: number | null; j?: number | null } | null | undefined) {
  const k = finiteNumber(signal?.k);
  const d = finiteNumber(signal?.d);
  const j = finiteNumber(signal?.j);
  return k == null || d == null || j == null ? null : { k, d, j };
}

export function latestKdjFromSeries(series: TrendIndicatorPoint[]) {
  for (let index = series.length - 1; index >= 0; index -= 1) {
    const kdj = kdjFromSignal(series[index]);
    if (kdj) return kdj;
  }
  const bars = toDailyBars(series);
  const computed = computeKdj(bars);
  return computed.length ? computed[computed.length - 1] : null;
}

export function signalStatusLabel(status?: string): string {
  const labels: Record<string, string> = {
    neutral: "中性",
    positive: "偏强",
    negative: "偏弱",
    bullish: "看多",
    bearish: "看空",
    buy_setup: "买点预备",
    exit: "离场",
    uptrend: "上升趋势",
    hold: "持有",
    watch: "观察",
    oversold: "超卖",
  };
  return labels[String(status || "neutral")] || String(status || "中性");
}

export function signalStatusTone(status?: string): "positive" | "negative" | "warning" | "neutral" {
  switch (status) {
    case "positive":
    case "bullish":
    case "buy_setup":
    case "uptrend":
    case "hold":
      return "positive";
    case "negative":
    case "bearish":
    case "exit":
      return "negative";
    case "watch":
    case "oversold":
      return "warning";
    default:
      return "neutral";
  }
}

export function finiteNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === "") return null;
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : null;
}
