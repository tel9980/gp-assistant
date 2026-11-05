// 从 lib/tauri.ts 拆出的模块；对外仍由 lib/tauri.ts 统一再导出。
import { clampInt, normalizeStockCode, relationStatusLabel, relationTypeLabel, stockCodeDigits, uniqueCompactStrings, uniqueNotes } from "./format";
import { withAndroidNetworkOptions } from "./marketRefresh";
import { arrayRecords, asRecord } from "./net";
import { InvokeFn } from "./tauriRuntime";

export async function analyzeMobileStockNews(invoke: InvokeFn, payload: Record<string, unknown>): Promise<Record<string, unknown>> {
  const mobilePayload = withAndroidNetworkOptions({
    ...payload,
    mobile_fast: true,
    include_us_market_brief: false,
    max_items: Math.max(10, Number(payload.max_items || 24)),
  });
  const cached = await invoke<Record<string, unknown>>("api_news_rag", { payload: mobilePayload });
  if (Number(cached.message_count || 0) > 0 || (Array.isArray(cached.findings) && cached.findings.length)) {
    return appendResultNotes(cached, ["移动端已使用安卓短链路或本机缓存完成分析；如源站均失败，可导入桌面端同步包补充。"]);
  }
  try {
    const offline = await analyzeMobileNewsRag(invoke, payload);
    return appendResultNotes(offline, ["移动端本机消息缓存暂无可用内容，已改用手机端已导入的上下游 RAG 包。"]);
  } catch (offlineError) {
    const cachedNotes = Array.isArray(cached.notes) ? cached.notes.join(" ") : "";
    throw new Error(`${cachedNotes || "安卓在线短链路和本机缓存暂无可用内容。"} ${(offlineError as Error).message}`);
  }
}

export async function analyzeMobileNewsRag(invoke: InvokeFn, payload: Record<string, unknown>): Promise<Record<string, unknown>> {
  const seedCodes = Array.isArray(payload.seed_codes) ? payload.seed_codes : [];
  const requestedCode = normalizeStockCode(payload.code || seedCodes[0] || "");
  const detail = asRecord(await invoke("core_upstream_rag_detail", { payload: { stock_code: requestedCode, pack_version: "" } }));
  const manifest = asRecord(detail.manifest || detail);
  const manifestCode = normalizeStockCode(manifest.target_stock_code || "");
  if (requestedCode && manifestCode && stockCodeDigits(requestedCode) !== stockCodeDigits(manifestCode)) {
    throw new Error(`当前手机 RAG 包是 ${manifestCode}，不是 ${requestedCode}。请先导入目标股票的同步包。`);
  }
  const sources = mobileNewsRagSourcesFromManifest(manifest, Number(payload.max_items || 24));
  const skill = asRecord(await invoke("core_mobile_stock_skill", {
    payload: { stock_code: manifestCode || requestedCode, stock_name: manifest.target_stock_name || "", question: "分析上下游消息利好利空", sources },
  }));
  return mobileNewsSkillToNewsRagResult(skill, manifest, sources, Array.isArray(detail.notes) ? detail.notes as string[] : []);
}
export function mobileNewsRagSourcesFromManifest(manifest: Record<string, unknown>, maxItems = 24): Record<string, unknown>[] {
  const sources: Record<string, unknown>[] = [];
  for (const edge of arrayRecords(manifest.relation_edges)) {
    const evidence = String(edge.evidence_text || edge.source_ref || "").trim();
    if (!evidence) continue;
    sources.push({
      title: mobileRelationSourceTitle(edge),
      summary: evidence,
      source_tier: edge.source_tier || "manual_url",
      source_name: edge.source_name || "RAG relation",
      published_at: edge.published_at || null,
      source_url: edge.source_url || null,
      evidence,
    });
  }
  for (const chunk of arrayRecords(manifest.evidence_chunks)) {
    const evidence = String(chunk.evidence_text || chunk.text || chunk.title || "").trim();
    if (!evidence) continue;
    sources.push({
      title: chunk.title || "RAG evidence chunk",
      summary: evidence,
      source_tier: chunk.source_tier || "manual_url",
      source_name: chunk.source_name || "RAG evidence",
      published_at: chunk.published_at || null,
      source_url: chunk.source_url || null,
      evidence,
    });
  }
  const seen = new Set<string>();
  return sources.filter((source) => {
    const key = [source.title, source.source_name, source.evidence].join("\n");
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  }).slice(0, clampInt(maxItems, 1, 80, 24));
}

export function mobileRelationSourceTitle(edge: Record<string, unknown>): string {
  const sourceEntity = asRecord(edge.source_entity);
  const targetEntity = asRecord(edge.target_entity);
  const source = sourceEntity.entity_name || sourceEntity.stock_code || edge.source_code || "upstream";
  const target = targetEntity.entity_name || targetEntity.stock_code || edge.target_code || "target";
  return `${source} ${relationTypeLabel(edge.relation_type)} ${target}: ${relationStatusLabel(edge.status)}`;
}

export function mobileNewsSkillToNewsRagResult(skill: Record<string, unknown>, manifest: Record<string, unknown>, sources: Record<string, unknown>[], detailNotes: string[]): Record<string, unknown> {
  const overview = asRecord(skill.overview);
  const code = normalizeStockCode(overview.stock_code || manifest.target_stock_code || "");
  const name = String(overview.stock_name || manifest.target_stock_name || "");
  const target = `${name || code || "目标股票"}${code ? ` (${code})` : ""}`;
  const findings = [
    ...mobileSkillFindingsToNewsFindings(skill.positive_factors, "positive", target, code),
    ...mobileSkillFindingsToNewsFindings(skill.negative_factors, "negative", target, code),
    ...mobileSkillFindingsToNewsFindings(skill.neutral_information, "mixed", target, code),
    ...mobileSkillFindingsToNewsFindings(skill.unverified_leads, "uncertain", target, code),
  ];
  return splitResultNotes({
    scope_codes: code ? [code] : [],
    relation_count: manifest.relation_edge_count ?? arrayRecords(manifest.relation_edges).length,
    message_count: sources.length,
    findings,
    notes: uniqueCompactStrings([
      String(overview.summary || ""),
      "移动端使用已导入本机 RAG 包离线分析，不会在手机端抓取公告或新闻。",
      sources.length ? "" : "当前 RAG 包没有可分析证据，请在桌面端重建包含证据片段的同步包。",
      ...detailNotes,
    ]),
  });
}

export function mobileSkillFindingsToNewsFindings(items: unknown, direction: string, target: string, code: string): Record<string, unknown>[] {
  return arrayRecords(items).map((item) => ({
    target,
    direction,
    confidence: confidenceLabel(item.confidence),
    impact_chain: item.summary || item.risk_note || item.title || "",
    evidence: [{
      title: item.title || "-",
      source: item.source_name || "local rag",
      source_tier: item.source_tier || "manual_url",
      published_at: item.published_at || null,
      url: item.source_url || null,
      stock_codes: code ? [code] : [],
      relation_types: [],
      sentiment: direction,
    }],
    pending_checks: uniqueCompactStrings([String(item.risk_note || "")]),
  }));
}

export function confidenceLabel(value: unknown): string {
  const score = Number(value);
  if (!Number.isFinite(score)) return "low";
  if (score >= 0.75) return "high";
  if (score >= 0.5) return "medium";
  return "low";
}

export function appendResultNotes<T extends Record<string, unknown>>(result: T, notes: string[]): T {
  return splitResultNotes({ ...result, notes: uniqueNotes([...(Array.isArray(result.notes) ? result.notes as string[] : []), ...notes]) });
}

export function splitResultNotes<T extends Record<string, unknown>>(result: T): T {
  const notes = Array.isArray(result.notes) ? result.notes.map(String).filter(Boolean) : [];
  const debugNotes = Array.isArray(result.debug_notes) ? result.debug_notes.map(String).filter(Boolean) : [];
  const visible: string[] = [];
  const debug = [...debugNotes];
  for (const note of notes) {
    if (isDebugNote(note)) debug.push(note);
    else visible.push(note);
  }
  return {
    ...result,
    notes: uniqueNotes(visible),
    debug_notes: uniqueNotes(debug),
  };
}

export function isDebugNote(note: string): boolean {
  return [
    /^Android /i,
    /^RAG 只在/,
    /^当前范围没有供应链/,
    /^当前为个股消息模式/,
    /^上下游 RAG 请使用/,
    /^未接入模型/,
    /^没有可分析目标/,
    /^消息缓存:/,
    /news-cache\.json/i,
    /GP_NEWS_ENABLE_TRADITIONAL_MEDIA/,
  ].some((pattern) => pattern.test(note));
}
