// 从 AgentPanel.tsx 拆出的纯逻辑/子组件；由原文件按需 import。
import { useCallback, useEffect, useState } from "react";
import { AgentMode } from "../../lib/agent";
import { normalizeAgentResult, normalizeAgentStreamEvent, parseSseBlock } from "../../lib/contracts";
// 经 lib/tauri 门面取运行时能力：测试会 mock "../../lib/tauri"，直接 import tauriRuntime 会绕过 mock。
import { buildTauriAgentPayload, getTauriInvoke, getTauriListen, isTauriRuntime } from "../../lib/tauri";
import { AgentResult, AgentStreamEvent } from "../../types/index";
import { AGENT_FAILED_LEDGER_DELETION_PREFIX, AGENT_HISTORY_KEY, AGENT_MODES, AgentConversation, AgentStep, ChatMessage, MAX_AGENT_CONVERSATIONS, MAX_AGENT_RUN_ID_BYTES, conversationDeletionCompletedIds } from "./agentPanelShared";
export function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

export function mergeStep(steps: AgentStep[], step: AgentStep): AgentStep[] {
  const next = [...steps];
  const index = next.findIndex((item) => item.stage === step.stage);
  if (index >= 0) next[index] = step;
  else next.push(step);
  return next;
}
export function updateAssistantProgress(
  conversationId: string,
  updateConversation: (conversationId: string, updater: (conversation: AgentConversation) => AgentConversation) => void,
  step: AgentStep,
  content: string,
) {
  updateConversation(conversationId, (conversation) => ({
    ...conversation,
    messages: conversation.messages.map((message, index) => {
      if (index !== conversation.messages.length - 1) return message;
      return {
        ...message,
        content,
        steps: mergeStep(message.steps || [], step),
      };
    }),
    updatedAt: Date.now(),
  }));
}

export function sanitizeLegacyAgentReply(content: string): string {
  const original = String(content || "").trim();
  if (!original) return "";
  const lines = original
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const kept = lines.filter((line) => !isAgentNoiseLine(line));
  return kept.length ? kept.join("\n") : original;
}

export function isAgentNoiseLine(line: string): boolean {
  return [
    /^Android short/i,
    /^Android .* returned \d+ items/i,
    /^RAG 只在/i,
    /^未接入模型/,
    /^当前范围没有供应链/,
    /^消息缓存:/,
    /^移动端已使用/,
    /^暂未找到相关股票/,
    /stocktopicer[\\/]+news[\\/]+news-cache\.json/i,
  ].some((pattern) => pattern.test(line));
}

export function createConversation(mode: AgentMode = "quick"): AgentConversation {
  const now = Date.now();
  return {
    id: crypto.randomUUID?.() || `agent-conversation-${now}`,
    title: "新对话",
    mode,
    messages: [],
    createdAt: now,
    updatedAt: now,
  };
}

export function usePersistentLedgerDeletionIds(): [string[], string[], () => void] {
  const [pendingIds, setPendingIds] = useState<string[]>(() => Array.from(persistedPendingLedgerDeletionIds()));
  const [markerIds, setMarkerIds] = useState<string[]>(() => Array.from(persistedLedgerDeletionIds()));
  const sync = useCallback(() => {
    setPendingIds(Array.from(persistedPendingLedgerDeletionIds()));
    setMarkerIds(Array.from(persistedLedgerDeletionIds()));
  }, []);
  useEffect(() => {
    const handleStorage = (event: StorageEvent) => {
      if (event.key === null || event.key.startsWith(AGENT_FAILED_LEDGER_DELETION_PREFIX)) sync();
    };
    window.addEventListener?.("storage", handleStorage);
    return () => window.removeEventListener?.("storage", handleStorage);
  }, [sync]);
  return [pendingIds, markerIds, sync];
}

export function ledgerDeletionTombstoneKey(conversationId: string): string {
  return `${AGENT_FAILED_LEDGER_DELETION_PREFIX}${encodeURIComponent(conversationId)}`;
}

export function prepareConversationDeletion(conversationId: string): boolean {
  try {
    localStorage.setItem(ledgerDeletionTombstoneKey(conversationId), "pending");
    return true;
  } catch {
    return false;
  }
}

export function persistLedgerDeletionCompletion(conversationId: string): boolean {
  try {
    localStorage.setItem(ledgerDeletionTombstoneKey(conversationId), "completed");
    return true;
  } catch {
    return false;
  }
}

export function persistConversationRemoval(conversationId: string): boolean {
  try {
    const stored = localStorage.getItem(AGENT_HISTORY_KEY);
    const conversations = stored ? JSON.parse(stored) : [];
    if (!Array.isArray(conversations)) return false;
    const remaining = conversations.filter((conversation) => (
      String(conversation?.id || "").trim() !== conversationId
    ));
    localStorage.setItem(
      AGENT_HISTORY_KEY,
      JSON.stringify(remaining.length ? remaining : [createConversation()]),
    );
    return true;
  } catch {
    return false;
  }
}

export function isBoundedAgentIdentity(value: string): boolean {
  return value.length > 0 && new TextEncoder().encode(value).byteLength <= MAX_AGENT_RUN_ID_BYTES;
}

export function sanitizeAgentConversations(value: AgentConversation[]): AgentConversation[] {
  if (!Array.isArray(value)) return [createConversation()];
  const pendingDeletionIds = persistedLedgerDeletionIds();
  const sanitized = value
    .filter((item) => {
      if (!item || typeof item !== "object") return false;
      const id = String(item.id || "").trim();
      return !conversationDeletionCompletedIds.has(id) && !pendingDeletionIds.has(id);
    })
    .map((item) => {
      const mode = AGENT_MODES.some((modeItem) => modeItem.id === item.mode) ? item.mode : "quick";
      const storedId = String(item.id || "").trim();
      const id = isBoundedAgentIdentity(storedId)
        ? storedId
        : crypto.randomUUID?.() || `agent-conversation-${Date.now()}`;
      // Persist only a lightweight transcript: drop the heavy `result` payload and the
      // transient `steps` progress list. Screen/backtest/news results can be large and
      // would otherwise blow past the localStorage quota; the reply text in `content`
      // is enough to reconstruct the conversation on reload.
      const messages: ChatMessage[] = Array.isArray(item.messages)
        ? item.messages
            .filter((message) => message?.role === "user" || message?.role === "assistant")
            .map((message) => ({
              role: message.role,
              content: String(message.content || ""),
              timestamp: Number(message.timestamp || Date.now()),
              ...(typeof message.runId === "string" && isBoundedAgentIdentity(message.runId.trim())
                ? { runId: message.runId.trim() }
                : {}),
              error: Boolean(message.error),
            }))
        : [];
      return {
        id,
        title: String(item.title || titleFromMessages(messages) || "新对话"),
        mode,
        messages,
        createdAt: Number(item.createdAt || Date.now()),
        updatedAt: Number(item.updatedAt || item.createdAt || Date.now()),
      };
    })
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, MAX_AGENT_CONVERSATIONS);
  return sanitized.length ? sanitized : [createConversation()];
}

export function persistedLedgerDeletionIds(status?: "pending" | "completed"): Set<string> {
  const ids = new Set<string>();
  try {
    for (let index = 0; index < localStorage.length; index += 1) {
      const key = localStorage.key(index);
      if (!key?.startsWith(AGENT_FAILED_LEDGER_DELETION_PREFIX)) continue;
      if (status && localStorage.getItem(key) !== status) continue;
      const encodedId = key.slice(AGENT_FAILED_LEDGER_DELETION_PREFIX.length);
      let id: string;
      try {
        id = decodeURIComponent(encodedId).trim();
      } catch {
        continue;
      }
      if (isBoundedAgentIdentity(id) && !conversationDeletionCompletedIds.has(id)) ids.add(id);
    }
  } catch {
    return ids;
  }
  return ids;
}

export function persistedPendingLedgerDeletionIds(): Set<string> {
  return persistedLedgerDeletionIds("pending");
}

export function titleFromMessages(messages: ChatMessage[]): string {
  return titleFromMessage(messages.find((message) => message.role === "user")?.content || "");
}

export function titleFromMessage(message: string): string {
  const compact = message.replace(/\s+/g, " ").trim();
  if (!compact) return "新对话";
  return compact.length > 24 ? `${compact.slice(0, 24)}...` : compact;
}

export function formatConversationTime(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  if (sameDay) return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  return date.toLocaleDateString("zh-CN", { month: "2-digit", day: "2-digit" });
}

export async function requestAgentStream(
  payload: Record<string, unknown>,
  onEvent: (event: AgentStreamEvent) => void,
  signal: AbortSignal,
): Promise<void> {
  if (isTauriRuntime()) return requestTauriAgentStream(payload, onEvent, signal);
  return requestDesktopAgentStream(payload, onEvent, signal);
}

export async function requestDesktopAgentStream(
  payload: Record<string, unknown>,
  onEvent: (event: AgentStreamEvent) => void,
  signal: AbortSignal,
): Promise<void> {
  const response = await fetch("/api/agent/stream", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
    signal,
  });
  if (!response.ok) throw new Error(await response.text() || `HTTP ${response.status}`);
  if (!response.body?.getReader) throw new Error("当前浏览器无法读取流式响应。");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const blocks = buffer.split(/\r?\n\r?\n/);
    buffer = blocks.pop() || "";
    for (const block of blocks) {
      const event = parseSseBlock(block);
      if (event) onEvent(event);
    }
  }
  const trailing = parseSseBlock(buffer);
  if (trailing) onEvent(trailing);
}

export async function requestTauriAgentStream(
  payload: Record<string, unknown>,
  onEvent: (event: AgentStreamEvent) => void,
  signal: AbortSignal,
): Promise<void> {
  const invoke = getTauriInvoke();
  const listen = getTauriListen();
  if (!invoke || !listen) throw new Error("Tauri 事件桥不可用。");
  const runId = String(payload.run_id || "");
  let sawResult = false;
  let unlisten: (() => void) | undefined;
  let removeAbortListener: (() => void) | undefined;
  try {
    unlisten = await listen("agent-stream-event", (event) => {
      const normalized = normalizeAgentStreamEvent(event);
      if (!normalized || normalized.run_id !== runId) return;
      if (normalized.type === "result") sawResult = true;
      onEvent(normalized);
    });
    const invocation = invoke<AgentResult>("api_agent_stream", {
      payload: buildTauriAgentPayload({ ...payload, run_id: runId }),
    });
    const aborted = new Promise<never>((_, reject) => {
      const abort = () => {
        void Promise.resolve(invoke("api_agent_cancel", { payload: { run_id: runId } })).catch(() => undefined);
        const error = new Error("已取消");
        error.name = "AbortError";
        reject(error);
      };
      if (signal.aborted) abort();
      else {
        signal.addEventListener("abort", abort, { once: true });
        removeAbortListener = () => signal.removeEventListener("abort", abort);
      }
    });
    const response = await Promise.race([invocation, aborted]);
    if (!sawResult && response) onEvent({ run_id: runId, type: "result", response: normalizeAgentResult(response) });
  } finally {
    removeAbortListener?.();
    unlisten?.();
  }
}
