import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, Menu, MessageSquareText, RefreshCw, Search, Send, X } from "lucide-react";

import { formatResearchUpdatedAt, groupMessages, isToday } from "./newsRagModel";
import { Answers, EventGroup, EvidencePanel, InboxPanel, KnowledgeDrawer, ResearchBriefCounts, ResearchEmptyState, ResearchPortal, ResearchSurfaceSheet, ResearchThreads } from "./newsRagSections";
import type {
  LlmSettings, ResearchAnswer, ResearchCitation,
  ResearchIndexStatus, ResearchMessage, ResearchOverview, ResearchQueryResult,
  ResearchThread, WatchlistItem,
} from "../../types";
import { buildLlmConfig, buildNewsRagRequest } from "../../lib/contracts";
import { hasErrorCode } from "../../lib/appError";
import { normalizeStockCode } from "../../lib/format";
import { getJson, isMobileTauriRuntime, postJson } from "../../lib/tauri";
import { applyMarkRead, pushCitation, useEventSelection } from "../../lib/newsInteractions";
import { LlmSettingsPanel } from "./LlmSettingsPanel";
import { PanelFeedback } from "../ui/PanelFeedback";
import { useMobileComposer } from "../../hooks/useMobileComposer";
import { useMediaQuery } from "../../hooks/useMediaQuery";
import { Sheet } from "../ui/Sheet";
import { ResearchMore } from "./ResearchMore";
import { StockCodeInput } from "../StockCodeInput";
import { useWorkspaceState, useWorkspaceStore, useWorkspaceCredentialsReady, WorkspaceSaveStatus } from "../../hooks/useWorkspace";

interface NewsRagPanelProps {
  llmSettings?: LlmSettings | null;
  onLlmSettingsChange?: import("../../lib/llmCredentialStore").LlmSettingsSetter;
  watchlist?: WatchlistItem[];
  initialCode?: string;
  initialCodeRequestId?: number;
  code?: string;
  onCodeChange?: (code: string) => void;
  onGoToScreen?: () => void;
  embedded?: boolean;
  active?: boolean;
  onHeaderToolsChange?: (tools: {
    refreshing: boolean;
    vectorReady: boolean;
    onRefresh: () => void;
    onOpenKnowledge: () => void;
    onOpenInbox: () => void;
  }) => void;
}
interface ThreadDetail { answers?: ResearchAnswer[]; }
interface QuestionFailure { title: string; message: string; threadId?: string; }
const EMPTY_WATCHLIST: WatchlistItem[] = [];

export function NewsRagPanel(props: NewsRagPanelProps) {
  const workspace = useWorkspaceStore();
  const credentialsReady = useWorkspaceCredentialsReady();
  const mobile = useMediaQuery("(max-width: 768px)");
  const nativeMobile = isMobileTauriRuntime();
  const active = props.active !== false;
  const questionInputId = useId();
  const watchlist = props.watchlist || EMPTY_WATCHLIST;
  const controlled = props.onCodeChange !== undefined;
  const [uncontrolledCode, setUncontrolledCode] = useWorkspaceState("news.stock", normalizeStockCode(props.initialCode || watchlist[0]?.code || ""));
  const code = controlled ? normalizeStockCode(props.code || "") : uncontrolledCode;
  const codeRef = useRef(code);
  const [overview, setOverview] = useState<ResearchOverview | null>(null);
  const [messages, setMessages] = useState<ResearchMessage[]>([]);
  const overviewRef = useRef<ResearchOverview | null>(null);
  const messagesRef = useRef<ResearchMessage[]>([]);
  const [threads, setThreads] = useState<ResearchThread[]>([]);
  const [threadId, setThreadId] = useWorkspaceState("news.thread", "");
  const threadIdRef = useRef("");
  const threadLoadRequestRef = useRef(0);
  const deletedThreadIdsRef = useRef(new Set<string>());
  const workspaceGenerationRef = useRef(0);
  const [answers, setAnswers] = useState<ResearchAnswer[]>([]);
  const [question, setQuestion] = useWorkspaceState("news.question", "");
  const [citationStack, setCitationStack] = useState<ResearchCitation[]>([]);
  const [citationPointer, setCitationPointer] = useState(-1);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [asking, setAsking] = useState(false);
  const [error, setError] = useState("");
  const [questionError, setQuestionError] = useState<QuestionFailure | null>(null);
  const [inboxOpen, setInboxOpen] = useState(false);
  const [questionsOpen, setQuestionsOpen] = useState(false);
  const [dataStatusOpen, setDataStatusOpen] = useState(false);
  const [modelOpenRequest, setModelOpenRequest] = useState(0);
  const modelReturnFocusRef = useRef<HTMLElement | null>(null);
  const [search, setSearch] = useState(code);
  const questionSurfaceRef = useRef<HTMLDivElement>(null);
  const questionPositionRef = useRef({ scroll: 0, start: 0, end: 0 });
  const citationOriginRef = useRef<HTMLElement | null>(null);
  const evidenceSurfaceRef = useRef<HTMLDivElement>(null);
  const [knowledgeOpen, setKnowledgeOpen] = useState(false);
  const [indexStatus, setIndexStatus] = useState<ResearchIndexStatus | null>(null);
  const [managementBusy, setManagementBusy] = useState(false);
  const [managementResult, setManagementResult] = useState<unknown>(null);
  const [deletingThreadId, setDeletingThreadId] = useState<string | null>(null);
  const [deleteCandidateId, setDeleteCandidateId] = useState<string | null>(null);
  const [readNotice, setReadNotice] = useState("");
  const [readNoticePulse, setReadNoticePulse] = useState(false);
  const [highlightAnswerId, setHighlightAnswerId] = useState<string | null>(null);
  const composer = useMobileComposer(question);
  const questionInputRef = composer.textareaRef;
  const answersRef = useRef<HTMLElement | null>(null);
  const eventRefs = useRef(new Map<string, HTMLButtonElement>());
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const noticeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { selectedId, toggle: toggleEvent, close: closeEvent } = useEventSelection();
  const citation = evidenceOpen && citationPointer >= 0 ? citationStack[citationPointer] || null : null;
  const activeLlmConfig = useMemo(() => buildLlmConfig(props.llmSettings), [props.llmSettings]);

  useEffect(() => { threadIdRef.current = threadId; }, [threadId]);
  const selectCode = useCallback((nextCode: string) => {
    const normalized = normalizeStockCode(nextCode);
    if (normalized === codeRef.current) return;
    codeRef.current = normalized;
    workspaceGenerationRef.current += 1;
    setAsking(false);
    setRefreshing(false);
    setOverview(null);
    overviewRef.current = null;
    setMessages([]);
    messagesRef.current = [];
    setThreadId("");
    threadIdRef.current = "";
    setAnswers([]);
    closeEvent();
    setCitationStack([]);
    setCitationPointer(-1);
    setEvidenceOpen(false);
    setError("");
    setQuestionError(null);
    setLoading(true);
    if (controlled) props.onCodeChange?.(normalized);
    else setUncontrolledCode(normalized);
  }, [closeEvent, controlled, props.onCodeChange, setThreadId, setUncontrolledCode]);
  useEffect(() => { if (codeRef.current !== code) selectCode(code); setSearch(code); }, [code, selectCode]);
  useEffect(() => {
    if (controlled) return;
    const next = normalizeStockCode(props.initialCode);
    if (next) selectCode(next);
  }, [controlled, props.initialCode, props.initialCodeRequestId, selectCode]);

  const loadThread = useCallback(async (
    id: string,
    generation = workspaceGenerationRef.current,
  ) => {
    if (!id) {
      if (generation === workspaceGenerationRef.current && !threadIdRef.current) setAnswers([]);
      return;
    }
    if (generation !== workspaceGenerationRef.current || threadIdRef.current !== id) return;
    const request = ++threadLoadRequestRef.current;
    const ownsRequest = () => generation === workspaceGenerationRef.current
      && threadIdRef.current === id && threadLoadRequestRef.current === request;
    setQuestionError((current) => current?.threadId === id ? null : current);
    try {
      const detail = await postJson<ThreadDetail>("/api/research/threads/detail", { thread_id: id });
      if (!ownsRequest()) return;
      setAnswers(detail.answers || []);
      setQuestionError((current) => current?.threadId === id ? null : current);
    } catch (nextError) {
      if (!ownsRequest()) return;
      if (hasErrorCode(nextError, "research.not_found")) {
        // 会话已经不在了（另一台设备删除、或数据库被切换/回滚）。
        // 继续把它留在侧栏只会让用户反复点到 404，直接从列表摘掉并说明原因。
        setThreads((current) => current.filter((item) => item.id !== id));
        threadIdRef.current = "";
        setThreadId("");
        setAnswers([]);
        setQuestionError({
          title: "会话已不存在",
          message: "这个会话可能已被删除，或研究数据库已切换。列表已刷新，请重新选择。",
        });
        return;
      }
      setQuestionError((current) => current && current.threadId !== id ? current : {
        title: "会话加载失败", message: (nextError as Error).message, threadId: id,
      });
    }
  }, []);

  const loadWorkspace = useCallback(async (quiet = false) => {
    const generation = ++workspaceGenerationRef.current;
    if (!quiet) setLoading(true);
    setError("");
    try {
      await workspace.initialize();
      if (generation !== workspaceGenerationRef.current) return;
      const messageQuery = code
        ? `?stock_code=${encodeURIComponent(code)}&limit=120`
        : "?limit=120";
      const [nextOverview, messageResult, threadResult] = await Promise.all([
        getJson<ResearchOverview>("/api/research/overview"),
        getJson<{ items?: ResearchMessage[] }>(`/api/research/messages${messageQuery}`),
        getJson<{ items?: ResearchThread[] }>("/api/research/threads"),
      ]);
      if (generation !== workspaceGenerationRef.current) return;
      const nextThreads = (threadResult.items || []).filter(
        (thread) => !deletedThreadIdsRef.current.has(thread.id),
      );
      overviewRef.current = nextOverview;
      messagesRef.current = messageResult.items || [];
      setOverview(nextOverview);
      setMessages(messagesRef.current);
      setThreads(nextThreads);
      const preferred = nextThreads.find((item) =>
        item.id === workspace.get("news.thread", threadIdRef.current) && (item.stock_code || "") === code)
        || nextThreads.find((item) => item.stock_code === code)
        || (!code ? nextThreads.find((item) => !item.stock_code) : undefined);
      if (preferred) {
        threadIdRef.current = preferred.id;
        setThreadId(preferred.id);
        await loadThread(preferred.id, generation);
      } else {
        threadIdRef.current = "";
        setThreadId("");
        setAnswers([]);
      }
    } catch (nextError) {
      if (generation === workspaceGenerationRef.current) {
        setError((nextError as Error).message);
      }
    } finally {
      if (!quiet && generation === workspaceGenerationRef.current) setLoading(false);
    }
  }, [code, loadThread, setThreadId, workspace]);

  useEffect(() => {
    void loadWorkspace();
    return () => { workspaceGenerationRef.current += 1; };
  }, [loadWorkspace]);

  const refresh = useCallback(async (background = false) => {
    const generation = workspaceGenerationRef.current;
    if (!code || !navigator.onLine || document.visibilityState !== "visible") {
      if (!background) await loadWorkspace();
      return;
    }
    if (!background) setRefreshing(true);
    try {
      await postJson("/api/research/refresh", buildNewsRagRequest(code, 30, undefined, watchlist));
      if (generation !== workspaceGenerationRef.current) return;
      await loadWorkspace(true);
    } catch (nextError) {
      if (!background && generation === workspaceGenerationRef.current) {
        setError((nextError as Error).message);
      }
    } finally {
      if (!background && generation === workspaceGenerationRef.current) setRefreshing(false);
    }
  }, [code, loadWorkspace, watchlist]);

  const visibleMessages = useMemo(
    () => code ? messages.filter((message) => message.stock_code === code) : messages,
    [code, messages],
  );
  const todayMessages = useMemo(
    () => visibleMessages.filter((message) => isToday(message.published_at)),
    [visibleMessages],
  );
  const grouped = useMemo(() => ({
    positive: todayMessages.filter((item) => ["positive", "bullish", "利好"].includes(item.sentiment)),
    negative: todayMessages.filter((item) => ["negative", "bearish", "利空"].includes(item.sentiment)),
    uncertain: todayMessages.filter((item) =>
      !["positive", "bullish", "利好", "negative", "bearish", "利空"].includes(item.sentiment)),
  }), [todayMessages]);
  const eventGroups = useMemo(() => groupMessages(visibleMessages), [visibleMessages]);
  const stock = watchlist.find((item) => normalizeStockCode(item.code) === code);
  const questionPlaceholder = code
    ? `询问 ${code} 的公告、财务或消息…`
    : "询问当前知识库…";
  const composerPlaceholder = mobile ? "询问公告、财务或消息…" : `${questionPlaceholder} · Ctrl+Enter 发送`;
  const summary = useMemo(() => {
    if (!todayMessages.length) return "今日暂无新增消息，可阅读已有资料。";
    const direction = grouped.positive.length > grouped.negative.length ? "利好线索较多"
      : grouped.negative.length > grouped.positive.length ? "风险线索较多" : "多空线索接近";
    return `${direction}，仍需结合公告和财务资料核查。`;
  }, [code, grouped, todayMessages.length]);

  const markReadBatch = useCallback(async (ids: string[], notice = false) => {
    const uniqueIds = [...new Set(ids)].filter(Boolean);
    if (!uniqueIds.length) return;
    try {
      await postJson("/api/research/mark-read", { message_ids: uniqueIds });
      const result = applyMarkRead(messagesRef.current, overviewRef.current, uniqueIds);
      messagesRef.current = result.messages;
      overviewRef.current = result.overview;
      setMessages(result.messages);
      setOverview(result.overview);
      if (notice) {
        setReadNotice("已全部标为已读");
        setReadNoticePulse(true);
        if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
        noticeTimerRef.current = setTimeout(() => {
          setReadNotice("");
          setReadNoticePulse(false);
        }, 2000);
      }
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }, []);

  const markReadStock = useCallback(async (stockCode: string) => {
    const normalizedStockCode = normalizeStockCode(stockCode);
    const ids = messagesRef.current.filter((message) => normalizeStockCode(message.stock_code) === normalizedStockCode && message.unread)
      .map((message) => message.id);
    const unreadCount = overviewRef.current?.unread_by_stock?.[normalizedStockCode] || 0;
    if (!ids.length && unreadCount <= 0) return;
    try {
      await postJson("/api/research/mark-read", { stock_code: normalizedStockCode });
      const nextMessages = messagesRef.current.map((message) =>
        ids.includes(message.id) ? { ...message, unread: false } : message,
      );
      const currentOverview = overviewRef.current;
      const nextOverview = currentOverview ? (() => {
        const unreadByStock = { ...(currentOverview.unread_by_stock || {}) };
        const changed = Math.max(unreadCount, ids.length);
        unreadByStock[normalizedStockCode] = 0;
        return {
          ...currentOverview,
          unread_count: Math.max(0, currentOverview.unread_count - changed),
          unread_by_stock: unreadByStock,
        };
      })() : currentOverview;
      messagesRef.current = nextMessages;
      overviewRef.current = nextOverview;
      setMessages(nextMessages);
      setOverview(nextOverview);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }, []);

  const rememberQuestions = useCallback(() => {
    const input = questionInputRef.current;
    if (!input || !questionSurfaceRef.current) return;
    questionPositionRef.current = { scroll: questionSurfaceRef.current?.scrollTop || 0, start: input?.selectionStart || 0, end: input?.selectionEnd || 0 };
  }, [questionInputRef]);
  const closeQuestions = useCallback(() => { rememberQuestions(); setQuestionsOpen(false); }, [rememberQuestions]);
  const pushCitationSelection = useCallback((next: ResearchCitation) => {
    setInboxOpen(false);
    if (!citation) {
      citationOriginRef.current = document.activeElement as HTMLElement | null;
      if (questionsOpen) rememberQuestions();
    }
    const result = pushCitation(citationStack, citationPointer, next);
    setCitationStack(result.stack);
    setCitationPointer(result.pointer);
    setEvidenceOpen(true);
  }, [citation, citationPointer, citationStack, questionsOpen, rememberQuestions]);

  const closeCitation = useCallback(() => {
    setEvidenceOpen(false);
    queueMicrotask(() => citationOriginRef.current?.focus?.());
  }, []);
  useEffect(() => {
    if (!active) return;
    if (citation) evidenceSurfaceRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    else if (questionsOpen) {
      const position = questionPositionRef.current;
      if (questionSurfaceRef.current) questionSurfaceRef.current.scrollTop = position.scroll;
      questionInputRef.current?.setSelectionRange?.(position.start, position.end);
      const surface = questionSurfaceRef.current;
      const input = questionInputRef.current;
      return () => {
        if (surface && input) questionPositionRef.current = { scroll: surface.scrollTop || 0, start: input.selectionStart || 0, end: input.selectionEnd || 0 };
      };
    }
  }, [active, citation, questionsOpen, questionInputRef]);

  const selectEvent = useCallback((message: ResearchMessage) => {
    const wasSelected = selectedId === message.id;
    toggleEvent(message.id);
    if (wasSelected) return;
    if (message.unread) void markReadBatch([message.id]);
  }, [markReadBatch, selectedId, toggleEvent]);

  useEffect(() => {
    if (!selectedId) return;
    const element = eventRefs.current.get(selectedId);
    if (!element || typeof element.scrollIntoView !== "function") return;
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    element.scrollIntoView({ block: "nearest", behavior: reducedMotion ? "auto" : "smooth" });
  }, [selectedId]);

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (citation || questionsOpen || inboxOpen || knowledgeOpen || event.defaultPrevented) return;
      if (selectedId) closeEvent();
    };
    window.addEventListener?.("keydown", handleEscape);
    return () => {
      window.removeEventListener?.("keydown", handleEscape);
    };
  }, [citation, questionsOpen, inboxOpen, knowledgeOpen, closeEvent, selectedId]);

  useEffect(() => {
    return () => {
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
      if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
    };
  }, []);

  const createThread = useCallback(async (
    generation = workspaceGenerationRef.current,
    requestedCode = code,
  ): Promise<string | null> => {
    const thread = await postJson<ResearchThread>("/api/research/threads/create", {
      title: requestedCode ? `${requestedCode} 研究` : "综合研究",
      stock_code: requestedCode || null,
    });
    if (generation !== workspaceGenerationRef.current) return null;
    threadIdRef.current = thread.id;
    threadLoadRequestRef.current += 1;
    setQuestionError(null);
    setThreads((current) => [thread, ...current]);
    setThreadId(thread.id);
    setAnswers([]);
    closeEvent();
    setInboxOpen(false);
    queueMicrotask(() => questionInputRef.current?.focus());
    return thread.id;
  }, [code, closeEvent]);

  const createQuestionThread = useCallback(async () => {
    const generation = workspaceGenerationRef.current;
    setQuestionError(null);
    try { await createThread(generation); }
    catch (nextError) {
      if (generation === workspaceGenerationRef.current) setQuestionError({ title: "新建会话失败", message: (nextError as Error).message });
    }
  }, [createThread]);

  const deleteThread = useCallback(async (thread: ResearchThread) => {
    if (asking || deletingThreadId) return;
    if (deleteCandidateId !== thread.id) {
      setDeleteCandidateId(thread.id);
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setDeleteCandidateId(null), 3000);
      return;
    }
    setDeleteCandidateId(null);
    const generation = workspaceGenerationRef.current;
    setDeletingThreadId(thread.id);
    setQuestionError(null);
    try {
      try {
        await postJson("/api/research/threads/delete", { thread_id: thread.id });
      } catch (deleteError) {
        // 后端说会话不存在 = 目标状态已达成，按删除成功继续走本地清理。
        if (!hasErrorCode(deleteError, "research.not_found")) throw deleteError;
      }
      deletedThreadIdsRef.current.add(thread.id);
      setThreads((current) => current.filter((item) => item.id !== thread.id));
      if (threadIdRef.current === thread.id) {
        threadIdRef.current = "";
        setThreadId("");
        setAnswers([]);
      }
      if (generation === workspaceGenerationRef.current) await loadWorkspace(true);
    } catch (nextError) {
      if (generation === workspaceGenerationRef.current) setQuestionError({ title: "删除会话失败", message: (nextError as Error).message });
    } finally {
      setDeletingThreadId(null);
    }
  }, [asking, deleteCandidateId, deletingThreadId, loadWorkspace]);

  const ask = useCallback(async () => {
    const text = question.trim();
    if (!text || asking || deletingThreadId || !credentialsReady) return;
    const generation = workspaceGenerationRef.current;
    const requestedCode = code;
    setAsking(true);
    setQuestionError(null);
    let failureTitle = "新建会话失败";
    try {
      const selectedThread = threads.find((thread) => thread.id === threadId);
      const activeThread = selectedThread && (selectedThread.stock_code || "") === requestedCode
        ? selectedThread.id
        : await createThread(generation, requestedCode);
      if (!activeThread || generation !== workspaceGenerationRef.current) return;
      failureTitle = "提问失败";
      const result = await postJson<ResearchQueryResult>("/api/research/query", {
        query: text, stock_code: requestedCode || null, thread_id: activeThread, top_k: 8,
        llm: activeLlmConfig,
      });
      if (generation !== workspaceGenerationRef.current) return;
      const answer: ResearchAnswer = { ...result, question: text, citations: result.citations || [] };
      const answerKey = answer.id || `${answer.question}-${answers.length}`;
      setAnswers((current) => [...current, answer]);
      setQuestion(current => current === question ? "" : current);
      void workspace.flush();
      setHighlightAnswerId(answerKey);
      if (answer.citations[0]) pushCitationSelection(answer.citations[0]);
      if (mobile) setInboxOpen(false);
      failureTitle = "会话列表更新失败";
      const nextThreads = await getJson<{ items?: ResearchThread[] }>("/api/research/threads");
      if (generation === workspaceGenerationRef.current) {
        setThreads((nextThreads.items || []).filter(
          (thread) => !deletedThreadIdsRef.current.has(thread.id),
        ));
      }
    } catch (nextError) {
      if (generation === workspaceGenerationRef.current) setQuestionError({ title: failureTitle, message: (nextError as Error).message });
    } finally {
      if (generation === workspaceGenerationRef.current) setAsking(false);
    }
  }, [activeLlmConfig, answers.length, asking, code, createThread, deletingThreadId, mobile, pushCitationSelection, question, threadId, threads, credentialsReady, setQuestion, workspace]);

  useEffect(() => {
    if (!highlightAnswerId) return;
    const element = answersRef.current;
    if (element && typeof element.scrollIntoView === "function") {
      const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
      element.scrollIntoView({ block: "end", behavior: reducedMotion ? "auto" : "smooth" });
    }
    const timer = setTimeout(() => setHighlightAnswerId(null), 600);
    return () => clearTimeout(timer);
  }, [answers.length, highlightAnswerId]);

  const management = useCallback(async (action: () => Promise<unknown>) => {
    setManagementBusy(true);
    setManagementResult(null);
    try {
      setManagementResult(await action());
      setIndexStatus(await getJson<ResearchIndexStatus>("/api/research/index-status"));
      await loadWorkspace(true);
    } catch (nextError) {
      setManagementResult({ error: (nextError as Error).message });
    } finally {
      setManagementBusy(false);
    }
  }, [loadWorkspace]);

  const openKnowledge = useCallback(() => {
    setKnowledgeOpen(true);
    void getJson<ResearchIndexStatus>("/api/research/index-status")
      .then(setIndexStatus)
      .catch((nextError) => setManagementResult({ error: (nextError as Error).message }));
  }, []);
  const vectorReady = overview?.retrieval?.vector?.ready === true;
  const onHeaderRefresh = useCallback(() => { void refresh(); }, [refresh]);
  const onHeaderInbox = useCallback(() => { closeCitation(); setInboxOpen(true); }, [closeCitation]);
  useEffect(() => {
    if (!props.embedded || !props.onHeaderToolsChange) return;
    props.onHeaderToolsChange({
      refreshing,
      vectorReady,
      onRefresh: onHeaderRefresh,
      onOpenKnowledge: openKnowledge,
      onOpenInbox: onHeaderInbox,
    });
  }, [onHeaderInbox, onHeaderRefresh, openKnowledge, props.embedded, props.onHeaderToolsChange, refreshing, vectorReady]);

  const unreadVisibleIds = visibleMessages.filter((message) => message.unread).map((message) => message.id);
  const lastUpdated = formatResearchUpdatedAt(overview);

  return <section className="research-workspace" aria-label="研究消息中心">
    {loading && !overview && <p className="research-loading-status" role="status">正在打开研究消息中心，可继续编辑草稿…</p>}
    {!props.embedded && <header className="research-topbar">
      <form className="sentiment-toolbar" onSubmit={(event) => { event.preventDefault(); const next = normalizeStockCode(search); if (/^\d{6}\.(SH|SZ|BJ)$/.test(next)) selectCode(next); else setError("请输入六位股票代码，或从搜索结果中选择股票。"); }}>
        <StockCodeInput id={`${questionInputId}-stock`} value={search} onChange={setSearch} onCommit={selectCode} placeholder="代码或名称" inputAriaLabel="股票代码或名称" />
        <button type="submit" className="sentiment-icon-action" aria-label="查看股票消息"><Search size={16} /></button>
      </form>
      <div className="research-actions">
        <button type="button" className="research-mobile-stock-button" aria-label="打开股票列表" onClick={() => setInboxOpen(true)}><Menu size={16} /><span>股票</span></button>
        <button type="button" onClick={() => void refresh()} disabled={refreshing}><RefreshCw size={15} /><span>{refreshing ? "更新中" : "更新"}</span></button>
        <ResearchMore onKnowledge={openKnowledge} onModel={() => { modelReturnFocusRef.current = document.activeElement as HTMLElement | null; setModelOpenRequest((value) => value + 1); }} onStatus={() => setDataStatusOpen(true)} modelDisabled={!credentialsReady} />
        <div className="research-model-dialog-host"><LlmSettingsPanel settings={props.llmSettings || null}
          onChange={credentialsReady ? props.onLlmSettingsChange || (() => undefined) : () => undefined} presentation="dialog" statusMode="inline" openRequest={modelOpenRequest} returnFocusRef={modelReturnFocusRef} /></div>
      </div>
    </header>}

    {error && <div className="research-error">
      <PanelFeedback kind="error" title="研究中心暂时不可用" description={error} />
    </div>}

    <div className="research-columns">
      <ResearchSurfaceSheet active={mobile} open={active && inboxOpen} close={()=>setInboxOpen(false)} label="自选股收件箱">
      <InboxPanel code={code} setCode={(next) => { selectCode(next); setInboxOpen(false); }}
        watchlist={watchlist} unreadByStock={overview?.unread_by_stock || {}}
        unread={overview?.unread_count || 0} open={inboxOpen} close={() => setInboxOpen(false)} />
      </ResearchSurfaceSheet>

      <main className="research-stream">
        <div className="research-stream-body">
        <div className="research-reading-heading">
          <h1>{stock?.name || (code ? "股票消息" : "全部消息")}{code && <small>{code}</small>}</h1>
          <button type="button" className="research-ghost-button" aria-label="打开研究问答" aria-expanded={questionsOpen}
            onClick={() => { setInboxOpen(false); setQuestionsOpen(true); }}><MessageSquareText size={16} aria-hidden="true" />研究问答</button>
        </div>
        {visibleMessages.length > 0 && <details className="research-daily-brief">
          <summary className="research-brief-summary"><span>{summary}</span><strong>今日新增 {todayMessages.length} 条</strong><ChevronRight className="research-brief-chevron" size={16} aria-hidden="true" /></summary>
          <div className="research-brief-expanded"><ResearchBriefCounts positive={grouped.positive.length} negative={grouped.negative.length} uncertain={grouped.uncertain.length} />
            <p>{lastUpdated ? `上次更新 ${lastUpdated}` : "尚未记录更新时间"} · 每 15 分钟自动更新</p>
          </div>
        </details>}

        <section className="research-event-section">
          <div className="research-section-heading"><div><span>消息</span></div><div className="research-heading-actions">
              {readNotice && <small className={readNoticePulse ? "is-flashing" : ""}>{readNotice}</small>}
              {(unreadVisibleIds.length > 0 || (code && (overview?.unread_by_stock?.[code] || 0) > 0)) && <button type="button" className="research-ghost-button"
                onClick={() => code ? void markReadStock(code) : void markReadBatch(unreadVisibleIds, true)}>全部已读</button>}
              <span>{visibleMessages.length} 条</span>
            </div>
          </div>
          {loading ? null : !visibleMessages.length ? <ResearchEmptyState
            refreshing={refreshing}
            onRefresh={() => void refresh()}
            code={code}
            needsWatchlist={!code && watchlist.length === 0}
            onPickStock={() => mobile ? setInboxOpen(true) : selectCode(watchlist[0]?.code || "")}
            onGoToScreen={props.onGoToScreen}
          /> : <div className="research-event-list">
            {eventGroups.map((group) => <EventGroup key={group.key}
              label={group.label} tone={group.key} messages={group.messages}
              selectedId={selectedId} selectEvent={selectEvent}
              setEventRef={(id, element) => {
                if (element) eventRefs.current.set(id, element);
                else eventRefs.current.delete(id);
              }} pushCitation={pushCitationSelection} />)}
          </div>}
        </section>

        <p className="research-risk-boundary">仅供研究，不构成投资建议。</p>
        </div>
      </main>

      <Sheet open={active && (questionsOpen || Boolean(citation))} onClose={citation ? closeCitation : closeQuestions}
        label={citation ? "引用证据检查器" : "研究问答"} className="research-surface-sheet" backdropClassName="research-surface-backdrop">
        {questionsOpen && <div className="research-question-surface" hidden={Boolean(citation)} ref={questionSurfaceRef}>
          <div className="research-detail-heading"><h2>研究问答</h2><button type="button" className="research-icon-button" aria-label="关闭研究问答" onClick={closeQuestions}><X size={18} /></button></div>
          <ResearchThreads threads={threads.filter((item) => (item.stock_code || "") === code)} threadId={threadId} asking={asking} deletingThreadId={deletingThreadId} deleteCandidateId={deleteCandidateId}
            createThread={() => void createQuestionThread()} deleteThread={(thread) => void deleteThread(thread)}
            setThread={(thread) => { const generation = ++workspaceGenerationRef.current; setQuestionError(null); setAsking(false); setThreadId(thread.id); threadIdRef.current = thread.id; setAnswers([]); void workspace.flush(); void loadThread(thread.id, generation); }} />
          {questionError && <PanelFeedback kind="error" title={questionError.title} description={questionError.message}
            action={questionError.threadId ? <button type="button" className="research-ghost-button" onClick={() => void loadThread(questionError.threadId!, workspaceGenerationRef.current)}>重试加载会话</button> : undefined} />}
          <Answers answers={answers} pushCitation={pushCitationSelection} highlightedId={highlightAnswerId} sectionRef={answersRef} />
        <form className="research-composer" onSubmit={(event) => { event.preventDefault(); void ask(); }}>
          <div><label className="research-composer-label" htmlFor={questionInputId}>
            <span>研究问题</span>{activeLlmConfig && <small>模型回答会强制引用证据</small>}
          </label>
            <div className="research-composer-row">
              <textarea ref={questionInputRef} id={questionInputId} value={question}
                aria-label="研究问题"
                onChange={(event) => setQuestion(event.target.value)}
                onFocus={composer.onFocus} onBlur={composer.onBlur}
                onCompositionStart={composer.onCompositionStart} onCompositionEnd={composer.onCompositionEnd}
                onKeyDown={(event) => {
                  if (!composer.isComposing(event) && event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
                    event.preventDefault();
                    void ask();
                  }
                }}
                placeholder={composerPlaceholder} rows={2} />
              <button className="research-composer-send" type="submit"
                aria-label="提交问题；仅供研究，不构成投资建议。"
                title="提交问题；仅供研究，不构成投资建议。"
                disabled={!credentialsReady || !question.trim() || asking || Boolean(deletingThreadId)}>
                {asking || deletingThreadId
                  ? <RefreshCw size={18} className="is-spinning" /> : <Send size={18} />}
              </button>
            </div>
          </div>
          <WorkspaceSaveStatus />
          <div className="research-composer-meta">
            <p className="research-risk-boundary">仅供研究，不构成投资建议。</p>
            {!activeLlmConfig && <p className="research-composer-setup">未配置模型，将只用已导入资料回答</p>}
          </div>
        </form>

        </div>}
        {citation && <div className="research-evidence-surface" ref={evidenceSurfaceRef}>
          <button type="button" className="research-ghost-button research-evidence-back" aria-label={questionsOpen ? "返回研究问答" : "返回消息"} onClick={closeCitation}><ChevronLeft size={16} />{questionsOpen ? "返回研究问答" : "返回消息"}</button>
          <EvidencePanel citation={citation} citationIndex={citationPointer}
            citationCount={citationStack.length} onPrevious={() => setCitationPointer((value) => Math.max(0, value - 1))}
            onNext={() => setCitationPointer((value) => Math.min(citationStack.length - 1, value + 1))} close={closeCitation} />
        </div>}
      </Sheet>
    </div>

    <Sheet open={active && dataStatusOpen} onClose={() => setDataStatusOpen(false)} label="研究数据状态" className="research-surface-sheet" backdropClassName="research-surface-backdrop">
      <div className="research-detail-heading"><h2>数据状态</h2><button type="button" className="research-icon-button" aria-label="关闭数据状态" onClick={() => setDataStatusOpen(false)}><X size={18} /></button></div>
      <p>{vectorReady ? "关键词 + 语义检索" : "关键词检索"}</p><p>{lastUpdated ? `上次更新 ${lastUpdated}` : "尚未记录更新时间"}</p>
    </Sheet>
    {knowledgeOpen && <ResearchPortal><KnowledgeDrawer code={code} mobile={nativeMobile}
      status={indexStatus} management={management} busy={managementBusy}
      result={managementResult} close={() => setKnowledgeOpen(false)} /></ResearchPortal>}
  </section>;
}

// 拆分后保持对外导出面不变（其它面板与测试仍从这里取这些符号）。
export { EvidenceInspector, NewsRagView } from "./newsRagSections";
export { formatResearchUpdatedAt, researchOperationNotice, validateResearchFile } from "./newsRagModel";
