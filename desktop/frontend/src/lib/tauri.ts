import { OBSERVE_FULL_HISTORY_LIMIT, OBSERVE_PREFETCH_TIMEOUT_MS, refreshTauriMarketData, withAndroidNetworkOptions } from "./marketRefresh";
import { analyzeMobileStockNews } from "./mobileNews";
import { asRecord, withTimeout } from "./net";
import { fetchObserveDailyHistoryForTauriStable } from "./observeDaily";
import { invokeBacktest, invokeLocalData, invokeWithFinancialSnapshot, loadBundledFinancialSnapshotForCode } from "./snapshot";
import { InvokeFn, TauriRouteHandler, getTauriInvoke, isMobileTauriRuntime } from "./tauriRuntime";

import {
  clampInt,
  currentSystemDateCompact,
  normalizeDateParam,
  normalizeStockCode,
} from "./format";
import { hydrateObserveTrendFromHistory } from "./observeTrend";
import { parseMarketCapabilities } from "./marketCapabilities";
import { parseSecurityId } from "./marketDomain";
import type { HkObserveResult, ObserveResult } from "../types";
import {
  jsonByteLength,
  logPerfRecord,
  objectRecordCount,
  perfDiagnosticsAreEnabled,
  perfOutcomeForError,
} from "./perfDiagnostics";

declare global {
  interface Window {
    __TAURI__?: {
      core?: { invoke: <T = unknown>(command: string, args?: Record<string, unknown>) => Promise<T> };
      event?: { listen: (event: string, handler: (event: unknown) => void) => Promise<() => void> };
    };
    __TAURI_INTERNALS__?: unknown;
    __TAURI_IPC__?: unknown;
  }
}


export interface RequestOptions {
  timeoutMs?: number;
  signal?: AbortSignal | null;
  headers?: Record<string, string>;
}

export interface MarketRefreshLogEntry {
  time: string;
  message: string;
  tone: string;
}

export interface MarketRefreshOptions {
  signal?: AbortSignal;
  mode?: string;
  max_bytes?: number;
  daily_days?: number;
  minute_days?: number;
  batch_start?: number;
  batch_count?: number;
  max_candidates?: number;
  max_failed_batches?: number;
  max_refresh_loops?: number;
  prefetch_history_limit?: number;
  validate_after_write?: boolean;
  onLog?: (entry: MarketRefreshLogEntry) => void;
}

const STOCK_SEARCH_LIMIT = 5;
const MOBILE_OBSERVE_INVOKE_TIMEOUT_MS = 55000;
const MOBILE_NEWS_RAG_TIMEOUT_MS = 45000;
export const TAURI_GET_ROUTES: Record<string, TauriRouteHandler> = {
  "/api/stock-research/status": async ({ invoke }) => invoke("api_stock_research_status"),
  "/api/stock-research/report": async ({ invoke, parsed }) => invoke("api_stock_research_report", { payload: { run_id: parsed.searchParams.get("run_id") || "" } }),
  "/api/data/status": async ({ invoke }) => invoke("api_data_status"),
  "/health": async ({ invoke }) => invoke("api_health"),
  "/api/strategies": async ({ invoke }) => invoke("api_strategies"),
  "/api/data-sources": async ({ invoke }) => invoke("api_data_sources"),
  "/api/data-sources/status": async ({ invoke }) => invoke("api_market_status"),
  "/api/market/capabilities": async ({ invoke, parsed }) => {
    const market = parsed.searchParams.get("market");
    if (market !== "cn" && market !== "hk") {
      throw new Error("unsupported market; expected one of: cn, hk");
    }
    const response = await invoke<unknown>("api_market_capabilities", { market });
    const capabilities = parseMarketCapabilities(response, market);
    if (!capabilities) {
      throw new Error("native market capability registry returned an invalid response");
    }
    return capabilities;
  },
  "/api/hk/market-status": async ({ invoke }) => invoke("api_hk_market_status"),
  "/api/hk/stock-search": async ({ invoke, parsed }) => invoke("api_hk_stock_search", {
    payload: { query: parsed.searchParams.get("q") || "", limit: Number(parsed.searchParams.get("limit") || 20) },
  }),
  "/api/hk/financial-status": async ({ invoke, parsed }) => invoke("api_hk_financial_status", { payload: { code: parsed.searchParams.get("code") || "" } }),
  "/api/rag-pack/status": async ({ invoke }) => invoke("api_rag_pack_status"),
  "/api/research/overview": async ({ invoke, parsed }) => invoke("api_research_overview", {
    payload: { stock_code: parsed.searchParams.get("stock_code") || "" },
  }),
  "/api/research/messages": async ({ invoke, parsed }) => invoke("api_research_messages", {
    payload: {
      stock_code: parsed.searchParams.get("stock_code") || "",
      unread_only: parsed.searchParams.get("unread_only") === "true",
      limit: Number(parsed.searchParams.get("limit") || 100),
    },
  }),
  "/api/research/threads": async ({ invoke }) => invoke("api_research_threads"),
  "/api/research/index-status": async ({ invoke }) => invoke("api_research_index_status"),
  "/api/agent/prompt-overlays": async ({ invoke }) => invoke("api_agent_prompt_overlays"),
  "/api/evolution/settings": async ({ invoke }) => invoke("api_evolution_settings"),
  "/api/evolution/profile": async ({ invoke }) => invoke("api_evolution_profile"),
  "/api/sentiment/strategies": async ({ invoke }) => invoke("api_sentiment_strategies"),
  "/api/sentiment/strategies/versions": async ({ invoke, parsed }) => invoke("api_sentiment_strategy_versions", { payload: { strategy_id: parsed.searchParams.get("strategy_id") || "" } }),
  "/api/agent/gepa/status": async ({ invoke }) => invoke("api_agent_gepa_status"),
  "/api/agent/runs": async ({ invoke, parsed }) => invoke("api_agent_run_list", {
    payload: {
      conversation_id: parsed.searchParams.get("conversation_id") || "",
      limit: Number(parsed.searchParams.get("limit") || 50),
    },
  }),
  "/api/agent/metrics": async ({ invoke, parsed }) => invoke("api_agent_run_metrics", {
    payload: {
      conversation_id: parsed.searchParams.get("conversation_id") || "",
      limit: Number(parsed.searchParams.get("limit") || 200),
    },
  }),
  "/api/upstream-rag/mobile/list": async ({ invoke }) => invoke("core_upstream_rag_list"),
  "/api/upstream-rag/status": async ({ invoke }) => invoke("api_upstream_rag_status"),
  "/api/upstream-rag/mobile/detail": async ({ invoke, parsed }) => invoke("core_upstream_rag_detail", {
    payload: {
      stock_code: parsed.searchParams.get("stock_code") || "",
      pack_version: parsed.searchParams.get("pack_version") || "",
    },
  }),
  "/api/stock-search": async ({ invoke, parsed }) => invoke("api_stock_search", {
    payload: { q: parsed.searchParams.get("q") || "", limit: parsed.searchParams.get("limit") || STOCK_SEARCH_LIMIT },
  }),
};

export const TAURI_GET_PREFIX_ROUTES: { prefix: string; handler: TauriRouteHandler }[] = [
  {
    prefix: "/api/data/refresh/",
    handler: async ({ invoke, path }) => invoke("api_data_refresh_status", {
      runId: decodeURIComponent(path.slice("/api/data/refresh/".length)),
    }),
  },
  {
    prefix: "/api/agent/runs/",
    handler: async ({ invoke, path }) => invoke("api_agent_run_get", {
      payload: { run_id: decodeURIComponent(path.slice("/api/agent/runs/".length)) },
    }),
  },
  {
    prefix: "/api/stocks/",
    handler: async ({ invoke, path }) => invoke("api_stock_get", { payload: { code: decodeURIComponent(path.slice("/api/stocks/".length)) } }),
  },
  {
    prefix: "/api/minutes/",
    handler: async ({ invoke, path, parsed }) => invoke("api_minutes", {
      payload: {
        code: decodeURIComponent(path.slice("/api/minutes/".length)),
        start: parsed.searchParams.get("start") || "",
        end: parsed.searchParams.get("end") || "",
        period: parsed.searchParams.get("period") || "1",
        limit: parsed.searchParams.get("limit") || 500,
      },
    }),
  },
  {
    prefix: "/api/order-book/",
    handler: async ({ invoke, path }) => invoke("api_order_book", { payload: { code: decodeURIComponent(path.slice("/api/order-book/".length)) } }),
  },
  {
    prefix: "/api/observe/",
    handler: async ({ invoke, path, parsed }) => {
      const code = normalizeStockCode(decodeURIComponent(path.slice("/api/observe/".length)));
      const mobileRuntime = isMobileTauriRuntime();
      const perfEnabled = perfDiagnosticsAreEnabled(mobileRuntime);
      const payload: Record<string, unknown> = withAndroidNetworkOptions({
        code,
        data_policy: parsed.searchParams.get("data_policy") ?? "cache_only",
        ...(parsed.searchParams.has("request_id") ? { request_id: parsed.searchParams.get("request_id") } : {}),
        ...(parsed.searchParams.has("job_id") ? { job_id: parsed.searchParams.get("job_id") } : {}),
        start_date: normalizeDateParam(parsed.searchParams.get("start_date"), "19900101"),
        end_date: normalizeDateParam(parsed.searchParams.get("end_date"), currentSystemDateCompact()),
        series_limit: clampInt(parsed.searchParams.get("series_limit"), 20, OBSERVE_FULL_HISTORY_LIMIT, OBSERVE_FULL_HISTORY_LIMIT),
        include_order_book: parsed.searchParams.get("include_order_book") === "true",
        include_chip_distribution: parsed.searchParams.get("include_chip_distribution") !== "false",
      });
      if (perfEnabled) payload.perf_diagnostics = true;
      const cacheStrategy = payload.data_policy === "refresh" ? "refresh" : "cache_only";
      const logInvoke = async () => {
        const requestBytes = perfEnabled ? jsonByteLength(payload) : undefined;
        const startedAt = perfEnabled ? performance.now() : 0;
        try {
          const result = await invokeLocalData(invoke, "api_observe", payload);
          const responseRecord = asRecord(result);
          logPerfRecord({
            flow: "observe",
            stage: "ipc_roundtrip",
            elapsedMs: perfEnabled ? performance.now() - startedAt : 0,
            cacheStrategy,
            stockCount: responseRecord.stock ? 1 : 0,
            historyRows: Array.isArray(asRecord(responseRecord.trend).series)
              ? (asRecord(responseRecord.trend).series as unknown[]).length
              : 0,
            financialRecords: objectRecordCount(responseRecord.financial_indicators),
            requestBytes,
            responseBytes: perfEnabled ? jsonByteLength(result) : undefined,
            outcome: "ok",
          }, mobileRuntime);
          return result;
        } catch (error) {
          logPerfRecord({
            flow: "observe",
            stage: "ipc_roundtrip",
            elapsedMs: perfEnabled ? performance.now() - startedAt : 0,
            cacheStrategy,
            requestBytes,
            outcome: perfOutcomeForError(error),
          }, mobileRuntime);
          throw error;
        }
      };
      // Local reads skip WebView prefetch but still record the Rust command round trip.
      if (payload.data_policy !== "refresh") return logInvoke();
      let history: Record<string, unknown>[] | null = null;
      if (mobileRuntime) {
        payload.mobile_fast_observe = true;
      }
      const financialStartedAt = performance.now();
      let financialSnapshot: Record<string, unknown> | null = null;
      let financialOutcome: "ok" | "error" | "timeout" = "ok";
      try {
        financialSnapshot = await loadBundledFinancialSnapshotForCode(code);
      } catch (error) {
        financialOutcome = perfOutcomeForError(error);
      }
      if (financialSnapshot) payload.financial_snapshot = financialSnapshot;
      logPerfRecord({
        flow: "observe",
        stage: "financial_snapshot_prefetch",
        elapsedMs: performance.now() - financialStartedAt,
        cacheStrategy,
        stockCount: 1,
        financialRecords: objectRecordCount(financialSnapshot?.financials),
        outcome: financialOutcome,
      }, mobileRuntime);
      const historyStartedAt = performance.now();
      let historyOutcome: "ok" | "error" | "timeout" = "ok";
      try {
        history = await fetchObserveDailyHistoryForTauriStable(payload, OBSERVE_PREFETCH_TIMEOUT_MS);
      } catch (error) {
        historyOutcome = perfOutcomeForError(error);
      }
      if (Array.isArray(history) && history.length) payload.history = history;
      logPerfRecord({
        flow: "observe",
        stage: "history_prefetch",
        elapsedMs: performance.now() - historyStartedAt,
        cacheStrategy,
        stockCount: 1,
        historyRows: history?.length || 0,
        outcome: historyOutcome,
      }, mobileRuntime);

      const observeInvoke = logInvoke();
      if (!mobileRuntime) {
        const observeResult = await observeInvoke;
        const hydrateStartedAt = performance.now();
        const hydrated = hydrateObserveTrendFromHistory(observeResult as ObserveResult, code, history);
        logPerfRecord({
          flow: "observe",
          stage: "trend_hydration",
          elapsedMs: performance.now() - hydrateStartedAt,
          cacheStrategy,
          stockCount: 1,
          historyRows: history?.length || 0,
          outcome: "ok",
        }, mobileRuntime);
        return hydrated;
      }

      const observeResult = await withTimeout(
        observeInvoke,
        MOBILE_OBSERVE_INVOKE_TIMEOUT_MS,
        `移动端观察超过 ${Math.round(MOBILE_OBSERVE_INVOKE_TIMEOUT_MS / 1000)} 秒未返回，已中止等待。`,
      );
      const hydrateStartedAt = performance.now();
      const hydrated = hydrateObserveTrendFromHistory(
        observeResult as ObserveResult,
        code,
        history,
      );
      logPerfRecord({
        flow: "observe",
        stage: "trend_hydration",
        elapsedMs: performance.now() - hydrateStartedAt,
        cacheStrategy,
        stockCount: 1,
        historyRows: history?.length || 0,
        outcome: "ok",
      }, mobileRuntime);
      return hydrated;
    },
  },
  {
    prefix: "/api/hk/observe/",
    handler: async ({ invoke, path, parsed }) => {
      const result = await invoke<HkObserveResult>("api_hk_observe", {
        payload: withAndroidNetworkOptions({
          code: decodeURIComponent(path.slice("/api/hk/observe/".length)),
          data_policy: parsed.searchParams.get("data_policy") || "cache_only",
          ...(parsed.searchParams.has("request_id")
            ? { request_id: parsed.searchParams.get("request_id") }
            : {}),
        }),
      });
      if (result.market !== "hk"
        || result.security.classification_status !== "verified_ordinary_share"
        || !parseSecurityId("hk", result.security.code, "ordinary_share")) {
        throw new Error("港股普通股分类或证券身份未核验，不能进入观察。");
      }
      return result;
    },
  },
  {
    prefix: "/api/hk/financial-status/",
    handler: async ({ invoke, path }) => invoke("api_hk_financial_status", {
      payload: { code: decodeURIComponent(path.slice("/api/hk/financial-status/".length)) },
    }),
  },
];

export const TAURI_POST_ROUTES: Record<string, TauriRouteHandler> = {
  "/api/data/refresh": async ({ invoke, payload }) => invoke("api_data_refresh", {
    payload: withAndroidNetworkOptions(asRecord(payload)),
  }),
  "/api/data/resume-explicit": async ({ invoke }) => invoke("api_data_resume_explicit"),
  "/api/data/cleanup/preview": async ({ invoke, payload }) => invoke("api_data_cleanup_preview", { payload }),
  "/api/data/cleanup": async ({ invoke, payload }) => invoke("api_data_cleanup", { payload }),
  "/api/sentiment/snapshot": async ({ invoke, payload }) => invoke("api_sentiment_snapshot", { payload }),
  "/api/sentiment/start": async ({ invoke, payload }) => invoke("api_sentiment_start", { payload: withAndroidNetworkOptions(asRecord(payload)) }),
  "/api/sentiment/status": async ({ invoke, payload }) => invoke("api_sentiment_status", { payload }),
  "/api/sentiment/cancel": async ({ invoke, payload }) => invoke("api_sentiment_cancel", { payload }),
  "/api/sentiment/latest": async ({ invoke, payload }) => invoke("api_sentiment_latest", { payload }),
  "/api/sentiment/history": async ({ invoke, payload }) => invoke("api_sentiment_history", { payload }),
  "/api/sentiment/followup": async ({ invoke, payload }) => invoke("api_sentiment_followup", { payload: withAndroidNetworkOptions(asRecord(payload)) }),
  "/api/screen": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_screen", payload),
  "/api/sector-screen": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_sector_screen", payload),
  "/api/custom-screen": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_custom_screen", payload),
  "/api/graph-screen": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_graph_screen", payload),
  "/api/trend": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_trend_analyze", payload),
  "/api/trend-screen": async ({ invoke, payload }) => invokeWithFinancialSnapshot(invoke, "api_trend_screen", payload),
  "/api/backtest": async ({ invoke, payload }) => invokeBacktest(invoke, payload),
  "/api/news-rag": async ({ invoke, payload }) => isMobileTauriRuntime()
    ? withTimeout(analyzeMobileStockNews(invoke, asRecord(payload)), MOBILE_NEWS_RAG_TIMEOUT_MS, `移动端消息分析超过 ${Math.round(MOBILE_NEWS_RAG_TIMEOUT_MS / 1000)} 秒未返回，已中止等待。`)
    : invoke("api_news_rag", { payload }),
  "/api/research/refresh": async ({ invoke, payload }) => invoke("api_research_refresh", { payload }),
  "/api/research/mark-read": async ({ invoke, payload }) => invoke("api_research_mark_read", { payload }),
  "/api/research/query": async ({ invoke, payload }) => invoke("api_research_query", { payload }),
  "/api/research/threads/create": async ({ invoke, payload }) => invoke("api_research_thread_create", { payload }),
  "/api/research/threads/detail": async ({ invoke, payload }) => invoke("api_research_thread_detail", { payload }),
  "/api/research/threads/delete": async ({ invoke, payload }) => invoke("api_research_thread_delete", { payload }),
  "/api/research/rebuild-index": async ({ invoke }) => invoke("api_research_rebuild_index"),
  "/api/research/rebuild-embeddings": async ({ invoke }) => invoke("api_research_rebuild_embeddings"),
  "/api/research/import-url": async ({ invoke, payload }) => invoke("api_research_import_url", { payload }),
  "/api/research/import-pdf": async ({ invoke, payload }) => invoke("api_research_import_pdf", { payload }),
  "/api/research/pack/export": async ({ invoke, payload }) => invoke("api_research_pack_export", { payload }),
  "/api/research/pack/import": async ({ invoke, payload }) => invoke("api_research_pack_import", { payload }),
  "/api/research/pack/rollback": async ({ invoke }) => invoke("api_research_pack_rollback"),
  "/api/rag-pack/build": async ({ invoke, payload }) => invoke("api_rag_pack_build", { payload }),
  "/api/rag-pack/build-from-news-cache": async ({ invoke, payload }) => invoke("api_rag_pack_build_from_news_cache", { payload }),
  "/api/rag-pack/query": async ({ invoke, payload }) => invoke("api_rag_pack_query", { payload }),
  "/api/upstream-rag/build": async ({ invoke, payload }) => invoke("api_upstream_rag_build", { payload }),
  "/api/upstream-rag/transfer/start": async ({ invoke, payload }) => invoke("api_upstream_rag_transfer_start", { payload }),
  "/api/data-sources/refresh-universe": async ({ invoke, payload }) => refreshTauriMarketData(invoke, asRecord(payload)),
  "/api/hk/refresh": async ({ invoke }) => invoke("api_hk_market_refresh"),
  "/api/hk/financial-refresh": async ({ invoke, payload }) => invoke("api_hk_financial_refresh", { payload }),
  "/api/data-sources/prune-cache": async ({ invoke }) => {
    const result = await invoke("api_market_clear_cache");
    return result;
  },
  "/api/upstream-rag/mobile/import": async ({ invoke, payload }) => invoke("core_upstream_rag_import", { payload }),
  "/api/upstream-rag/mobile/detail": async ({ invoke, payload }) => invoke("core_upstream_rag_detail", { payload }),
  "/api/upstream-rag/mobile/rollback": async ({ invoke, payload }) => invoke("core_upstream_rag_rollback", { payload }),
  "/api/llm/models": async ({ invoke, payload }) => invoke("api_llm_models", {
    payload: withAndroidNetworkOptions(asRecord(payload)),
  }),
  "/api/llm/test": async ({ invoke, payload }) => invoke("api_llm_test", {
    payload: withAndroidNetworkOptions(asRecord(payload)),
  }),
  "/api/agent": async ({ invoke, payload }) => invokeAgent(invoke, payload),
  "/api/agent/stream": async ({ invoke, payload }) => invokeAgent(invoke, payload),
  "/api/agent/cancel": async ({ invoke, payload }) => invoke("api_agent_cancel", {
    payload: { run_id: String(asRecord(payload).run_id || "").trim() },
  }),
  "/api/agent/prompt-overlays/revert": async ({ invoke, payload }) => invoke("api_agent_prompt_overlay_revert", { payload }),
  "/api/evolution/settings": async ({ invoke, payload }) => invoke("api_evolution_settings", { payload }),
  "/api/evolution/profile/reset": async ({ invoke }) => invoke("api_evolution_profile_reset"),
  "/api/evolution/review": async ({ invoke, payload }) => invoke("api_evolution_review", { payload }),
  "/api/evolution/review/enhance": async ({ invoke, payload }) => invoke("api_evolution_review_enhance", { payload }),
  "/api/evolution/rules/confirm": async ({ invoke, payload }) => invoke("api_evolution_confirm_rule", { payload }),
  "/api/evolution/rules/edit": async ({ invoke, payload }) => invoke("api_evolution_edit_rule", { payload }),
  "/api/evolution/rules/status": async ({ invoke, payload }) => invoke("api_evolution_rule_status", { payload }),
  "/api/evolution/rules/delete": async ({ invoke, payload }) => invoke("api_evolution_delete_rule", { payload }),
  "/api/evolution/suppress-kind": async ({ invoke, payload }) => invoke("api_evolution_suppress_kind", { payload }),
  "/api/evolution/unsuppress-kind": async ({ invoke, payload }) => invoke("api_evolution_unsuppress_kind", { payload }),
  "/api/sentiment/strategies": async ({ invoke, payload }) => invoke("api_sentiment_strategy_save", { payload }),
  "/api/sentiment/strategies/status": async ({ invoke, payload }) => invoke("api_sentiment_strategy_status", { payload }),
  "/api/sentiment/strategies/rollback": async ({ invoke, payload }) => invoke("api_sentiment_strategy_rollback", { payload }),
  "/api/agent/gepa/start": async ({ invoke, payload }) => invoke("api_agent_gepa_start", { payload }),
  "/api/agent/gepa/cancel": async ({ invoke, payload }) => invoke("api_agent_gepa_cancel", { payload }),
  "/api/agent/gepa/report": async ({ invoke, payload }) => invoke("api_agent_gepa_report", { payload }),
  "/api/agent/gepa/apply": async ({ invoke, payload }) => invoke("api_agent_gepa_apply", { payload }),
  "/api/agent/runs/delete-conversation": async ({ invoke, payload }) => invoke("api_agent_run_delete_conversation", { payload }),
};
export function buildTauriAgentPayload(request: Record<string, unknown>): Record<string, unknown> {
  return {
    message: String(request.message || ""),
    run_id: String(request.run_id || `react-agent-${Date.now()}`),
    ...(typeof request.conversation_id === "string" && request.conversation_id.trim()
      ? { conversation_id: request.conversation_id.trim() }
      : {}),
    mode: String(request.mode || "quick"),
    context: request.context,
    platform: request.platform,
    network: request.network,
    llm: request.llm,
    history: request.history,
  };
}

function invokeAgent(invoke: InvokeFn, payload: unknown): Promise<unknown> {
  return invoke("api_agent_stream", { payload: buildTauriAgentPayload(asRecord(payload)) });
}

export async function cancelTauriAgentRun(runId: string): Promise<boolean> {
  const invoke = getTauriInvoke();
  const normalized = runId.trim();
  if (!invoke || !normalized) return false;
  const result = await invoke<{ cancelled?: unknown }>("api_agent_cancel", {
    payload: { run_id: normalized },
  });
  return result?.cancelled === true;
}



// 自 lib/tauri.ts 拆出的域模块，保持对外导出面不变（调用点无需改动）。
export * from "./tauriRuntime";
export * from "./net";
export * from "./mobileNews";
export * from "./observeDaily";
export * from "./snapshot";
export * from "./marketRefresh";

export function tauriRouteHandler(method: string, path: string): TauriRouteHandler | null {
  if (method === "GET") return TAURI_GET_ROUTES[path] || TAURI_GET_PREFIX_ROUTES.find((route) => path.startsWith(route.prefix))?.handler || null;
  if (method === "POST") return TAURI_POST_ROUTES[path] || null;
  return null;
}
