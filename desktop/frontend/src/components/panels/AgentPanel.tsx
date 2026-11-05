import { FileSearch, History, LoaderCircle, Menu, PanelLeftClose, PanelLeftOpen, Plus, RefreshCw, Search, Send, Square, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AgentStreamEvent, LlmSettings, StockRowView, WatchlistItem } from "../../types";
import { activeLlmProvider, buildLlmConfig, normalizeAgentResult } from "../../lib/contracts";
import { buildAgentStreamPayload, MAX_AGENT_MESSAGE_CHARS } from "../../lib/agent";
import { deleteAgentConversationRuns, type AgentRunDetail } from "../../lib/agentRuns";
import { GepaLabPanel } from "./GepaLabPanel";
import { useWorkspaceState, useWorkspaceStore, useWorkspaceStatus, useWorkspaceCredentialsReady, WorkspaceSaveStatus } from "../../hooks/useWorkspace";
import { useLocalStorage } from "../../hooks/useLocalStorage";
import { useMobileComposer } from "../../hooks/useMobileComposer";
import { AgentResultView } from "./AgentResultView";
import { AgentRunDrawer } from "./AgentRunDrawer";
import { LlmSettingsPanel } from "./LlmSettingsPanel";
import { IconButton } from "../ui/IconButton";
import { createEvolutionReview, extractEvolutionEvidenceIds, type EvolutionReview } from "../../lib/evolution";
import { EvolutionReviewCard } from "./EvolutionReviewCard";

import { asRecord } from "../../lib/net";
import { createConversation, formatConversationTime, persistConversationRemoval, persistLedgerDeletionCompletion, persistedLedgerDeletionIds, prepareConversationDeletion, requestAgentStream, sanitizeLegacyAgentReply, titleFromMessage, updateAssistantProgress, usePersistentLedgerDeletionIds } from "./agentPanelModel";
import { AgentMode } from "../../lib/agent";
import { AGENT_EXAMPLES, AGENT_MOBILE_DRAWER_QUERY, AGENT_MODES, AGENT_RAIL_COLLAPSED_KEY, AgentConversation, AgentStep, ChatMessage, ConversationDeletionEvent, MAX_AGENT_CONVERSATIONS, agentModeLabel, conversationDeletionInFlightIds, conversationDeletionListeners, publishConversationDeletion } from "./agentPanelShared";
interface AgentPanelProps {
  llmSettings: LlmSettings | null;
  onLlmSettingsChange: import("../../lib/llmCredentialStore").LlmSettingsSetter;
  watchlist: WatchlistItem[];
  onWatchlistChange: (items: WatchlistItem[]) => void;
  draftPrompt?: string;
  draftRequestId?: number;
}

export function AgentPanel({ llmSettings, onLlmSettingsChange, watchlist, onWatchlistChange, draftPrompt, draftRequestId }: AgentPanelProps) {
  const [failedLedgerDeletionIds, ledgerDeletionMarkerIds, syncLedgerDeletionIds] = usePersistentLedgerDeletionIds();
  const workspace = useWorkspaceStore();
  const workspaceStatus = useWorkspaceStatus();
  const credentialsReady = useWorkspaceCredentialsReady();
  const initialConversations = useMemo(() => [createConversation()], []);
  const [conversations, setConversations] = useWorkspaceState<AgentConversation[]>("agent.conversations", initialConversations);
  const [activeConversationId, setActiveConversationId] = useWorkspaceState("agent.active", "");
  const [railCollapsed, setRailCollapsed] = useLocalStorage<boolean>(AGENT_RAIL_COLLAPSED_KEY, false);
  const [settingsRequest, setSettingsRequest] = useState(0);
  const [conversationSearch, setConversationSearch] = useState("");
  const [runningConversationIds, setRunningConversationIds] = useState<string[]>([]);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [replayOpen, setReplayOpen] = useState(false);
  const [gepaEnabled, setGepaEnabled] = useState(false);
  const [gepaOpen, setGepaOpen] = useState(false);
  const [evolutionReview, setEvolutionReview] = useState<EvolutionReview>();
  const [evolutionReviewLoading, setEvolutionReviewLoading] = useState(false);
  const [replayRunId, setReplayRunId] = useState<string>();
  const [finishedRunId, setFinishedRunId] = useState<string>();
  const [finishedRunConversationId, setFinishedRunConversationId] = useState<string>();
  const [conversationDeleteStorageError, setConversationDeleteStorageError] = useState<string>();
  const [ledgerDeletionInFlightIds, setLedgerDeletionInFlightIds] = useState<string[]>(
    () => Array.from(conversationDeletionInFlightIds),
  );
  const [retryingLedgerDeletions, setRetryingLedgerDeletions] = useState(false);
  const [ledgerRevision, setLedgerRevision] = useState(0);
  const threadRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const replayTriggerRef = useRef<HTMLElement | null>(null);
  const activeRunsRef = useRef(new Map<string, { runId: string; controller: AbortController }>());
  const pendingDeleteRef = useRef<string | null>(null);
  const appliedDraftRef = useRef(0);
  const activeProvider = activeLlmProvider(llmSettings);
  const activeLlmConfig = useMemo(() => buildLlmConfig(llmSettings), [llmSettings]);

  useEffect(() => () => {
    for (const run of activeRunsRef.current.values()) run.controller.abort();
  }, []);

  const activeConversation = conversations.find((item) => item.id === activeConversationId) || conversations[0] || null;
  const [input, setDraft] = useWorkspaceState(`agent.draft:${activeConversation?.id || "compose"}`, "");
  const setInput = useCallback((value: string) => {
    if (!workspaceStatus.ready && activeConversation) {
      setActiveConversationId(activeConversation.id);
      setConversations(current => current);
    }
    setDraft(value);
  }, [activeConversation, setActiveConversationId, setConversations, setDraft, workspaceStatus.ready]);
  const composer = useMobileComposer(input);

  const activeConversationDeleting = Boolean(
    activeConversation
      && (ledgerDeletionInFlightIds.includes(activeConversation.id)
        || ledgerDeletionMarkerIds.includes(activeConversation.id)),
  );
  const activeMode = AGENT_MODES.find((item) => item.id === activeConversation?.mode) || AGENT_MODES[0];
  const currentRunning = Boolean(activeConversation?.id && runningConversationIds.includes(activeConversation.id));
  const messages = activeConversation?.messages || [];
  const modelRequired = activeMode.id !== "quick" && !activeLlmConfig;
  const sortedConversations = useMemo(
    () => [...conversations].sort((a, b) => b.updatedAt - a.updatedAt),
    [conversations],
  );
  const visibleConversations = useMemo(() => {
    const query = conversationSearch.trim().toLowerCase();
    if (!query) return sortedConversations;
    return sortedConversations.filter((conversation) => {
      const messageText = conversation.messages.map((message) => message.content).join(" ");
      return `${conversation.title} ${messageText}`.toLowerCase().includes(query);
    });
  }, [conversationSearch, sortedConversations]);
  const groupedConversations = useMemo(() => {
    const groups = [
      { label: "7 天内", items: [] as AgentConversation[] },
      { label: "30 天内", items: [] as AgentConversation[] },
      { label: "更早", items: [] as AgentConversation[] },
    ];
    const now = Date.now();
    const day = 24 * 60 * 60 * 1000;
    visibleConversations.forEach((conversation) => {
      const age = now - conversation.updatedAt;
      if (age <= 7 * day) {
        groups[0].items.push(conversation);
      } else if (age <= 30 * day) {
        groups[1].items.push(conversation);
      } else {
        groups[2].items.push(conversation);
      }
    });
    return groups.filter((group) => group.items.length);
  }, [visibleConversations]);

  useEffect(() => {
    if (!workspaceStatus.ready) return;
    if (!workspace.get<AgentConversation[]>("agent.conversations", []).length) setConversations(conversations);
    if (!activeConversation && conversations[0]) {
      setActiveConversationId(conversations[0].id);
    } else if (activeConversation && activeConversation.id !== activeConversationId) {
      setActiveConversationId(activeConversation.id);
    }
  }, [activeConversation, activeConversationId, conversations, setActiveConversationId, setConversations, workspace, workspaceStatus.ready]);

  const handleThreadScroll = useCallback(() => {
    const node = threadRef.current;
    if (!node) return;
    stickToBottomRef.current = node.scrollHeight - node.scrollTop - node.clientHeight < 48;
  }, []);

  useEffect(() => {
    stickToBottomRef.current = true;
  }, [activeConversationId]);

  useEffect(() => {
    const node = threadRef.current;
    if (!node || !stickToBottomRef.current) return;
    node.scrollTo({ top: node.scrollHeight });
  }, [activeConversationId, messages, runningConversationIds]);

  useEffect(() => {
    setReplayOpen(false);
    setReplayRunId(undefined);
    replayTriggerRef.current = null;
  }, [activeConversationId]);

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;

    const mobileDrawerQuery = window.matchMedia(AGENT_MOBILE_DRAWER_QUERY);
    const syncRailForViewport = () => {
      if (mobileDrawerQuery.matches) {
        setRailCollapsed(true);
      }
    };

    syncRailForViewport();
    mobileDrawerQuery.addEventListener?.("change", syncRailForViewport);
    mobileDrawerQuery.addListener?.(syncRailForViewport);

    return () => {
      mobileDrawerQuery.removeEventListener?.("change", syncRailForViewport);
      mobileDrawerQuery.removeListener?.(syncRailForViewport);
    };
  }, [setRailCollapsed]);

  const updateConversation = useCallback((conversationId: string, updater: (conversation: AgentConversation) => AgentConversation) => {
    setConversations((prev) => prev.map((conversation) => (
      conversation.id === conversationId ? updater(conversation) : conversation
    )), !activeRunsRef.current.has(conversationId));
  }, [setConversations]);

  const startNewChat = useCallback(() => {
    const next = createConversation("quick");
    setConversations((prev) => [next, ...prev].slice(0, MAX_AGENT_CONVERSATIONS));
    setActiveConversationId(next.id);
    workspace.set(`agent.draft:${next.id}`, "");
    void workspace.flush();
    if (typeof window !== "undefined" && window.matchMedia?.(AGENT_MOBILE_DRAWER_QUERY).matches) {
      setRailCollapsed(true);
    }
  }, [setActiveConversationId, setConversations, setRailCollapsed, workspace]);

  const switchConversation = useCallback((conversationId: string) => {
    setActiveConversationId(conversationId);
    void workspace.flush();
    pendingDeleteRef.current = null;
    setPendingDeleteId(null);
    if (typeof window !== "undefined" && window.matchMedia?.(AGENT_MOBILE_DRAWER_QUERY).matches) {
      setRailCollapsed(true);
    }
  }, [setActiveConversationId, setRailCollapsed, workspace]);

  const removeLocalConversation = useCallback((conversationId: string) => {
    workspace.set(`agent.draft:${conversationId}`, null);
    setConversations((prev) => {
      const remaining = prev.filter((conversation) => conversation.id !== conversationId);
      const next = remaining.length ? remaining : [createConversation()];
      setActiveConversationId((current) => current === conversationId ? next[0].id : current);
      return next;
    });
  }, [setActiveConversationId, setConversations, workspace]);

  useEffect(() => {
    if (!workspaceStatus.ready) return;
    for (const conversationId of ledgerDeletionMarkerIds) {
      removeLocalConversation(conversationId);
    }
  }, [ledgerDeletionMarkerIds, removeLocalConversation, workspaceStatus.ready]);

  useEffect(() => {
    const listener = (event: ConversationDeletionEvent) => {
      setLedgerDeletionInFlightIds(Array.from(conversationDeletionInFlightIds));
      syncLedgerDeletionIds();
      if (event.type === "deleted") {
        removeLocalConversation(event.conversationId);
        setLedgerRevision((revision) => revision + 1);
        setConversationDeleteStorageError(event.storageError
          ? "运行记录已清理，但对话历史未能持久删除。"
          : undefined);
      } else if (event.type === "settled" && event.ledgerChanged) {
        setLedgerRevision((revision) => revision + 1);
      }
    };
    conversationDeletionListeners.add(listener);
    setLedgerDeletionInFlightIds(Array.from(conversationDeletionInFlightIds));
    return () => {
      conversationDeletionListeners.delete(listener);
    };
  }, [removeLocalConversation, syncLedgerDeletionIds]);

  const removeConversation = useCallback((conversationId: string) => {
    if (activeRunsRef.current.has(conversationId) || conversationDeletionInFlightIds.has(conversationId)) return;
    publishConversationDeletion({ conversationId, type: "started" });
    setConversationDeleteStorageError(undefined);
    if (!prepareConversationDeletion(conversationId)) {
      setConversationDeleteStorageError("本地存储空间不足，对话未删除。");
      publishConversationDeletion({ conversationId, type: "settled" });
      return;
    }

    syncLedgerDeletionIds();
    persistConversationRemoval(conversationId);
    removeLocalConversation(conversationId);
    void deleteAgentConversationRuns(conversationId)
      .then(() => {
        const persisted = persistConversationRemoval(conversationId);
        if (persisted) persistLedgerDeletionCompletion(conversationId);
        publishConversationDeletion({
          conversationId,
          type: "deleted",
          ledgerChanged: true,
          storageError: !persisted,
        });
      })
      .catch(() => publishConversationDeletion({ conversationId, type: "settled" }));
  }, [removeLocalConversation, syncLedgerDeletionIds]);

  const retryFailedLedgerDeletions = useCallback(() => {
    if (retryingLedgerDeletions || failedLedgerDeletionIds.length === 0) return;
    const retryIds = [...failedLedgerDeletionIds];
    setRetryingLedgerDeletions(true);
    void Promise.allSettled(retryIds.map((conversationId) => deleteAgentConversationRuns(conversationId)))
      .then((results) => {
        const succeededIds = retryIds.filter((_, index) => results[index].status === "fulfilled");
        if (succeededIds.length === 0) return;
        for (const conversationId of succeededIds) {
          const persisted = persistConversationRemoval(conversationId);
          if (persisted) persistLedgerDeletionCompletion(conversationId);
          publishConversationDeletion({
            conversationId,
            type: "deleted",
            ledgerChanged: true,
            storageError: !persisted,
          });
        }
      })
      .finally(() => setRetryingLedgerDeletions(false));
  }, [failedLedgerDeletionIds, retryingLedgerDeletions]);

  const openEvolutionReview = useCallback(async (message: ChatMessage, messageIndex: number) => {
    if (!activeConversation || !message.runId || !message.result) return;
    setEvolutionReviewLoading(true);
    try {
      const previous = activeConversation.messages[messageIndex - 1];
      const citations = extractEvolutionEvidenceIds(message.result);
      setEvolutionReview(await createEvolutionReview({
        run_id: message.runId,
        conversation_id: activeConversation.id,
        question: previous?.role === "user" ? previous.content : "",
        answer: message.result.reply || message.content,
        evidence_ids: citations,
        tool_calls: message.result.tool_calls || [],
      }));
    } catch (cause) {
      setEvolutionReview({ review_id: `local-error-${Date.now()}`, run_id: message.runId, conversation_id: activeConversation.id, status: "error", review_mode: "deterministic", research_goal: "复盘失败", evidence_and_process: { evidence_ids: [], tool_calls: [] }, conclusion_quality: { answer: cause instanceof Error ? cause.message : String(cause), has_answer: true }, blind_spot_candidates: [], rule_candidates: [] });
    } finally {
      setEvolutionReviewLoading(false);
    }
  }, [activeConversation]);

  const openEvolutionReviewDetail = useCallback(async (detail: AgentRunDetail) => {
    if (!detail.result) return;
    setEvolutionReviewLoading(true);
    try {
      const citations = extractEvolutionEvidenceIds(detail.result);
      setEvolutionReview(await createEvolutionReview({ run_id: detail.runId, conversation_id: detail.conversationId, question: detail.question, answer: detail.result.reply || "", evidence_ids: citations, tool_calls: detail.result.tool_calls || [] }));
    } catch (cause) {
      setEvolutionReview({ review_id: `local-error-${Date.now()}`, run_id: detail.runId, conversation_id: detail.conversationId, status: "error", review_mode: "deterministic", research_goal: "复盘失败", evidence_and_process: { evidence_ids: [], tool_calls: [] }, conclusion_quality: { answer: cause instanceof Error ? cause.message : String(cause), has_answer: true }, blind_spot_candidates: [], rule_candidates: [] });
    } finally { setEvolutionReviewLoading(false); }
  }, []);

  const changeMode = useCallback((mode: AgentMode) => {
    if (!activeConversation) return;
    updateConversation(activeConversation.id, (conversation) => ({
      ...conversation,
      mode,
      updatedAt: Date.now(),
    }));
  }, [activeConversation, updateConversation]);

  const toggleWatchlist = useCallback((item: StockRowView) => {
    const exists = watchlist.some((w) => w.code === item.code);
    if (exists) {
      onWatchlistChange(watchlist.filter((w) => w.code !== item.code));
    } else {
      onWatchlistChange([
        { code: item.code, name: item.name, industry: item.industry, added_at: new Date().toISOString(), source: "agent" },
        ...watchlist,
      ]);
    }
  }, [onWatchlistChange, watchlist]);

  const openRunHistory = useCallback((trigger: HTMLElement) => {
    replayTriggerRef.current = trigger;
    setReplayRunId(undefined);
    setReplayOpen(true);
  }, []);

  const openRunReplay = useCallback((runId: string, trigger: HTMLElement) => {
    replayTriggerRef.current = trigger;
    setReplayRunId(runId);
    setReplayOpen(true);
  }, []);

  const closeRunReplay = useCallback(() => setReplayOpen(false), []);

  const openGepaFromHistory = useCallback(() => {
    closeRunReplay();
    setGepaOpen(true);
  }, [closeRunReplay]);
  const closeGepa = useCallback(() => setGepaOpen(false), []);

  const cancelActiveRun = useCallback(() => {
    const conversationId = activeConversation?.id;
    if (!conversationId) return;
    activeRunsRef.current.get(conversationId)?.controller.abort();
  }, [activeConversation?.id]);

  useEffect(() => {
    if (!draftRequestId || appliedDraftRef.current === draftRequestId) return;
    appliedDraftRef.current = draftRequestId;
    const next = createConversation("quick");
    setConversations((prev) => [next, ...prev].slice(0, MAX_AGENT_CONVERSATIONS));
    setActiveConversationId(next.id);
    const prompt = draftPrompt || "";
    workspace.set(`agent.draft:${next.id}`, prompt);
    void workspace.flush();
  }, [draftPrompt, draftRequestId, setActiveConversationId, setConversations, workspace]);

  const send = useCallback(async () => {
    const text = input.trim();
    const conversationId = activeConversation?.id;
    const mode = activeConversation?.mode || "quick";
    if (
      !text
      || !credentialsReady
      || (mode !== "quick" && !activeLlmConfig)
      || activeRunsRef.current.has(conversationId || "")
      || !conversationId
      || conversationDeletionInFlightIds.has(conversationId)
      || persistedLedgerDeletionIds().has(conversationId)
    ) return;

    const runId = crypto.randomUUID?.() || `agent-${Date.now()}`;
    const now = Date.now();
    const userMessage: ChatMessage = { role: "user", content: text, timestamp: now };
    const assistantMessage: ChatMessage = { role: "assistant", content: "准备中...", timestamp: now, runId, steps: [] };
    const controller = new AbortController();
    activeRunsRef.current.set(conversationId, { runId, controller });
    setRunningConversationIds(Array.from(activeRunsRef.current.keys()));
    setInput("");

    updateConversation(conversationId, (conversation) => {
      const nextMessages = [...conversation.messages, userMessage, assistantMessage];
      return {
        ...conversation,
        title: conversation.messages.length ? conversation.title : titleFromMessage(text),
        messages: nextMessages,
        updatedAt: now,
      };
    });

    workspace.persist("agent.conversations");
    void workspace.flush();

    const patchAssistant = (patch: Partial<ChatMessage>) => {
      updateConversation(conversationId, (conversation) => ({
        ...conversation,
        messages: conversation.messages.map((message, index) => (
          index === conversation.messages.length - 1 ? { ...message, ...patch } : message
        )),
        updatedAt: Date.now(),
      }));
    };

    const applyEvent = (event: AgentStreamEvent) => {
      if (event.run_id && event.run_id !== runId) return;
      if (event.type === "status") {
        const step = {
          stage: event.stage || `stage-${Date.now()}`,
          label: event.label || event.stage || "运行中",
          percent: Number(event.percent || 0),
        };
        updateAssistantProgress(conversationId, updateConversation, step, event.label || event.stage || "运行中...");
      } else if (event.type === "tool_start") {
        const payload = asRecord(event.payload);
        const tool = String(payload.tool || event.action || "tool");
        const step = {
          stage: String(payload.id || `tool-${tool}`),
          label: String(payload.label || payload.tool || tool || "执行工具"),
          percent: 45,
        };
        updateAssistantProgress(conversationId, updateConversation, step, step.label);
      } else if (event.type === "tool_result") {
        const payload = asRecord(event.payload);
        const tool = String(payload.tool || event.action || "tool");
        const step = {
          stage: String(payload.id || `tool-${tool}`),
          label: String(payload.output_summary || payload.status || "工具完成"),
          percent: 78,
        };
        updateAssistantProgress(conversationId, updateConversation, step, step.label);
      } else if (event.type === "evidence") {
        updateAssistantProgress(conversationId, updateConversation, {
          stage: "evidence",
          label: "汇总证据",
          percent: 86,
        }, "正在汇总证据...");
      } else if (event.type === "final") {
        updateAssistantProgress(conversationId, updateConversation, {
          stage: "final",
          label: "整理结果",
          percent: 96,
        }, "正在整理结果...");
      } else if (event.type === "result") {
        const result = normalizeAgentResult(event.response || {});
        patchAssistant({ content: String(result.reply || "已完成。"), result, steps: undefined });
      } else if (event.type === "error") {
        patchAssistant({ content: event.message || "智能体执行失败。", error: true, steps: undefined });
      }
    };

    try {
      const payload = buildAgentStreamPayload({
        message: text,
        runId,
        conversationId,
        llm: activeLlmConfig,
        mode,
        watchlist,
        history: messages.map((message) => ({ role: message.role, content: message.content })),
      });
      if (!payload) return;
      await requestAgentStream(payload, applyEvent, controller.signal);
      setFinishedRunConversationId(conversationId);
      setFinishedRunId(runId);
    } catch (err) {
      const error = err as Error;
      patchAssistant(controller.signal.aborted || error.name === "AbortError"
        ? { content: "已取消。", error: false, steps: undefined }
        : { content: `错误：${error.message}`, error: true, steps: undefined });
      setFinishedRunConversationId(conversationId);
      setFinishedRunId(runId);
    } finally {
      if (activeRunsRef.current.get(conversationId)?.controller === controller) {
        activeRunsRef.current.delete(conversationId);
      }
      workspace.persist("agent.conversations");
      void workspace.flush();
      setRunningConversationIds(Array.from(activeRunsRef.current.keys()));
    }
  }, [activeConversation?.id, activeConversation?.mode, activeLlmConfig, input, messages, updateConversation, watchlist, credentialsReady, setInput, workspace]);

  const composerCard = (
    <div className="agent-composer-card">
      <div className="agent-composer-alerts">
        {conversationDeleteStorageError && (
          <div className="agent-quota-warning" role="alert">{conversationDeleteStorageError}</div>
        )}
        {failedLedgerDeletionIds.length > 0 && (
          <div className="agent-quota-warning agent-cleanup-warning" role="alert">
            <span>{failedLedgerDeletionIds.length} 个已删除对话的运行记录仍待清理。</span>
            <IconButton
              className="agent-cleanup-retry"
              onClick={retryFailedLedgerDeletions}
              disabled={retryingLedgerDeletions || ledgerDeletionInFlightIds.length > 0}
              label="重试清理运行记录"
              icon={retryingLedgerDeletions
                ? <LoaderCircle size={15} className="spin" aria-hidden="true" />
                : <RefreshCw size={15} aria-hidden="true" />}
            />
          </div>
        )}
      </div>
      {workspaceStatus.state === "error" && <WorkspaceSaveStatus />}
      <textarea
        ref={composer.textareaRef}
        className="agent-input"
        value={input}
        maxLength={MAX_AGENT_MESSAGE_CHARS}
        onChange={(event) => setInput(event.target.value)}
        onFocus={composer.onFocus}
        onBlur={composer.onBlur}
        onCompositionStart={composer.onCompositionStart}
        onCompositionEnd={composer.onCompositionEnd}
        onKeyDown={(event) => {
          if (!composer.mobile && !composer.isComposing(event) && event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            send();
          }
        }}
        placeholder="输入研究问题…"
        rows={3}
        disabled={currentRunning || activeConversationDeleting}
      />
      <div className="agent-composer-tools">
        <div className="agent-mode-tabs" role="group" aria-label="对话模式">
          {AGENT_MODES.map((item) => (
            <button
              key={item.id}
              type="button"
              className={item.id === activeMode.id ? "active" : ""}
              aria-pressed={item.id === activeMode.id}
              aria-label={`${item.label}：${item.hint}`}
              title={item.hint}
              onClick={() => changeMode(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>
      <p className="workspace-boundary">仅供研究，不构成投资建议。</p>
      <div className="agent-composer-footer">
        <button
          type="button"
          className="send-btn"
          onClick={currentRunning ? cancelActiveRun : send}
          disabled={!currentRunning && (!credentialsReady || activeConversationDeleting || !input.trim() || modelRequired)}
          aria-label={currentRunning ? "停止" : "发送"}
        >
          {currentRunning ? <Square size={17} aria-hidden="true" /> : <Send size={18} aria-hidden="true" />}
        </button>
      </div>
    </div>
  );

  return (
    <div className={`panel-container agent-panel agent-workspace ${railCollapsed ? "rail-collapsed" : ""}`}>
      <aside className="agent-rail" aria-label="Agent 会话历史">
        <div className="agent-rail-header">
          <div className="agent-rail-brand">
            <span className="agent-logo-mark">股</span>
            <div><strong>股选优</strong></div>
          </div>
          <IconButton
            className="agent-rail-toggle"
            onClick={() => setRailCollapsed((collapsed) => !collapsed)}
            aria-expanded={!railCollapsed}
            label={railCollapsed ? "展开会话历史" : "折叠会话历史"}
            icon={railCollapsed ? <PanelLeftOpen size={18} aria-hidden="true" /> : <PanelLeftClose size={18} aria-hidden="true" />}
          />
        </div>

        <label className="agent-rail-search">
          <Search size={15} aria-hidden="true" />
          <input
            aria-label="搜索对话内容"
            value={conversationSearch}
            onChange={(event) => setConversationSearch(event.target.value)}
            placeholder="搜索对话"
          />
        </label>

        <button type="button" className="agent-new-chat" aria-label="新建对话" title="新建对话" onClick={startNewChat}>
          <Plus size={16} aria-hidden="true" /><strong className="agent-new-chat-label">新建对话</strong>
        </button>

        <div className="agent-rail-section">
          <div className="agent-history-list">
            {groupedConversations.length ? groupedConversations.map((group) => (
              <div key={group.label} className="agent-history-group">
                <span className="agent-history-group-label">{group.label}</span>
                {group.items.map((conversation) => (
                  <article
                    key={conversation.id}
                    className={`agent-history-item ${conversation.id === activeConversation?.id ? "active" : ""}`}
                  >
                    <button
                      type="button"
                      className="agent-history-main"
                      aria-label={`${conversation.title || "新对话"}，${formatConversationTime(conversation.updatedAt)}，${agentModeLabel(conversation.mode)}，${conversation.messages.length ? `${conversation.messages.length} 条消息` : "未开始"}`}
                      title={`${conversation.title || "新对话"} · ${agentModeLabel(conversation.mode)} · ${formatConversationTime(conversation.updatedAt)}`}
                      onClick={() => switchConversation(conversation.id)}
                    >
                      <span>{conversation.title || "新对话"}</span>
                    </button>
                    <button
                      type="button"
                      className={`agent-history-remove${pendingDeleteId === conversation.id ? " is-confirming" : ""}`}
                      aria-label={pendingDeleteId === conversation.id ? "确认删除" : "删除对话"}
                      title={pendingDeleteId === conversation.id ? "确认删除" : "删除对话"}
                      disabled={runningConversationIds.includes(conversation.id) || ledgerDeletionInFlightIds.includes(conversation.id)}
                      onClick={(event) => {
                        event.stopPropagation();
                        if (pendingDeleteRef.current !== conversation.id) {
                          pendingDeleteRef.current = conversation.id;
                          setPendingDeleteId(conversation.id);
                          return;
                        }
                        pendingDeleteRef.current = null;
                        setPendingDeleteId(null);
                        removeConversation(conversation.id);
                      }}
                    >
                      {pendingDeleteId === conversation.id ? "确认" : <Trash2 size={14} aria-hidden="true" />}
                    </button>
                  </article>
                ))}
              </div>
            )) : (
              <div className="agent-history-empty">没有匹配的对话</div>
            )}
          </div>
        </div>

        <div className="agent-rail-footer agent-rail-model-settings">
          <fieldset disabled={!credentialsReady} style={{ border: 0, margin: 0, padding: 0 }}><LlmSettingsPanel settings={llmSettings} onChange={onLlmSettingsChange} presentation="dialog" openRequest={settingsRequest} /></fieldset>
        </div>
      </aside>

      <section className="agent-chat-stage" aria-label={`Agent 对话：${activeConversation?.title || "新对话"}，${activeMode.label}${activeProvider?.model ? `，模型 ${activeProvider.model}` : ""}`}>
        <button
          type="button"
          className="agent-rail-scrim"
          onClick={() => setRailCollapsed(true)}
          aria-label="关闭对话栏"
        />
        <div className="agent-thread-toolbar">
          <div className="agent-mobile-actions" aria-label="Agent 移动端操作">
            <IconButton
              className="agent-mobile-menu"
              onClick={() => setRailCollapsed(false)}
              label="展开对话栏"
              icon={<Menu size={18} aria-hidden="true" />}
            />
            <IconButton
              className="agent-mobile-new"
              onClick={() => {
                startNewChat();
                setRailCollapsed(true);
              }}
              label="新建对话"
              icon={<Plus size={18} aria-hidden="true" />}
            />
            <IconButton
              className="agent-mobile-history"
              onClick={(event) => openRunHistory(event.currentTarget)}
              label="复盘"
              title="复盘"
              icon={<History size={18} aria-hidden="true" />}
            />
          </div>
          <strong>{activeConversation?.title || "新对话"}</strong>
          <button
            type="button"
            className="icon-button agent-thread-history"
            onClick={(event) => openRunHistory(event.currentTarget)}
            aria-label="复盘"
            title="复盘"
          >
            <History size={17} aria-hidden="true" />
            <span>复盘</span>
          </button>
        </div>
        <div
          className={`agent-thread ${messages.length === 0 ? "empty" : ""}`}
          ref={threadRef}
          onScroll={handleThreadScroll}
        >
          {messages.length === 0 ? (
            <div className="agent-empty-layout">
              {!input.trim() && (
                <AgentEmptyState
                  mode={activeMode.id}
                  activeModel={activeLlmConfig?.model}
                  onExample={setInput}
                  onConfigureModel={() => setSettingsRequest((value) => value + 1)}
                />
              )}
              {composerCard}
            </div>
          ) : messages.map((msg, i) => (
            <article key={`${msg.timestamp}-${i}`} className={`agent-message ${msg.role} ${msg.error ? "error" : ""}`}>
              <div className="agent-message-meta">
                <span>{msg.role === "user" ? "你" : "Agent"}</span>
                <time>{new Date(msg.timestamp).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}</time>
                {msg.role === "assistant" && msg.runId && (
                  <>
                    <IconButton className="agent-message-replay" onClick={(event) => openRunReplay(msg.runId!, event.currentTarget)} label="运行记录" title="运行记录" icon={<FileSearch size={15} aria-hidden="true" />} />
                    <IconButton className="agent-message-review" onClick={() => void openEvolutionReview(msg, i)} label="复盘本次研究" title="复盘本次研究" icon={<History size={15} aria-hidden="true" />} />
                  </>
                )}
              </div>
              <div className="agent-message-body">
                {msg.steps?.length ? <AgentSteps steps={msg.steps} /> : (
                  <p className="agent-final-reply">{msg.role === "assistant" && !msg.result ? sanitizeLegacyAgentReply(msg.content) : msg.content}</p>
                )}
                {msg.result && <AgentResultView result={msg.result} watchlist={watchlist} onToggleWatchlist={toggleWatchlist} />}
              </div>
            </article>
          ))}
        </div>
        {messages.length > 0 && composerCard}
        <AgentRunDrawer
          open={replayOpen}
          activeConversationId={activeConversation?.id}
          initialRunId={replayRunId}
          initialTab="runs"
          finishedRunId={finishedRunId}
          finishedRunConversationId={finishedRunConversationId}
          ledgerRevision={ledgerRevision}
          gepaEnabled={gepaEnabled}
          onOpenGepa={openGepaFromHistory}
           onReviewRun={openEvolutionReviewDetail}
          returnFocusElement={replayTriggerRef.current}
          watchlist={watchlist}
          onToggleWatchlist={toggleWatchlist}
          onClose={closeRunReplay}
        />
        <GepaLabPanel
          llm={activeLlmConfig}
          open={gepaOpen}
          onClose={closeGepa}
          onAvailabilityChange={setGepaEnabled}
        />
        {evolutionReviewLoading && <div className="evolution-review-loading" role="status">正在生成研究复盘…</div>}
        {evolutionReview && <EvolutionReviewCard review={evolutionReview} llm={activeLlmConfig} onClose={() => setEvolutionReview(undefined)} />}
      </section>
    </div>
  );
}

function AgentEmptyState({
  mode,
  activeModel,
  onExample,
  onConfigureModel,
}: {
  mode: AgentMode;
  activeModel?: string;
  onExample: (example: string) => void;
  onConfigureModel: () => void;
}) {
  const needsModel = mode !== "quick" && !activeModel;
  return (
    <div className="agent-empty-state">
      <div className="agent-empty-copy">
        <h2>有什么可以帮忙？</h2>
      </div>
      <div className="agent-examples">
        {AGENT_EXAMPLES[mode].map((example) => (
          <button
            key={example}
            type="button"
            disabled={needsModel}
            onClick={() => { if (!needsModel) onExample(example); }}
          >
            {example}
          </button>
        ))}
      </div>
      {!activeModel && (
        <div className="agent-model-gate">
          <p role="status">模型未配置；快速模式用本地数据。</p>
          <button type="button" className="action-btn" onClick={onConfigureModel}>配置模型</button>
        </div>
      )}
    </div>
  );
}
function AgentSteps({ steps }: { steps: AgentStep[] }) {
  const latest = steps[steps.length - 1];
  if (!latest) return null;
  return <p className="agent-stream-status" role="status">{latest.label}</p>;
}

// 测试与其它模块历史上从这里取这些符号，拆分后保持导出面不变。
export { resetAgentConversationDeletionCoordinatorForTests } from "./agentPanelShared";
export { sanitizeAgentConversations } from "./agentPanelModel";
