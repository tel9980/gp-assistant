// 从 AgentPanel.tsx 拆出的纯逻辑/子组件；由原文件按需 import。
import { AgentResult } from "../../types/index";
export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  runId?: string;
  result?: AgentResult;
  steps?: AgentStep[];
  error?: boolean;
}

export interface AgentStep {
  stage: string;
  label: string;
  percent: number;
}

export interface AgentConversation {
  id: string;
  title: string;
  mode: AgentMode;
  messages: ChatMessage[];
  createdAt: number;
  updatedAt: number;
}

export const AGENT_MODES = [
  { id: "quick", label: "快速模式", hint: "选股 · 观察 · 消息" },
  { id: "expert", label: "专家模式", hint: "环境 · 主线 · 情绪" },
  { id: "research", label: "研报模式", hint: "质量 · 配置 · 估值" },
] as const;

export type AgentMode = typeof AGENT_MODES[number]["id"];
export const AGENT_EXAMPLES: Record<AgentMode, string[]> = {
  quick: ["帮我筛选今天值得看的趋势股", "看一下自选股里哪些偏离了原有逻辑", "查询 600519 最近的公告和新闻"],
  expert: ["当前市场情绪周期处在什么阶段，依据是什么", "哪条主线最强，什么证据会让它失效", "用游资早期框架看 600519，指出环境、主线和失效条件"],
  research: ["用价值复利框架评估 600519 的企业质量", "这家公司的资本配置有什么值得担心的地方", "当前估值假设是否经得起盈利下修"],
};
export function agentModeLabel(mode: string | undefined) {
  return AGENT_MODES.find((item) => item.id === mode)?.label || "快速模式";
}

export const AGENT_HISTORY_KEY = "stock-optimizer-agent-conversations";
export const AGENT_RAIL_COLLAPSED_KEY = "stock-optimizer-agent-rail-collapsed";
export const AGENT_FAILED_LEDGER_DELETION_PREFIX = "stock-optimizer-agent-failed-ledger-deletion:";
export const AGENT_MOBILE_DRAWER_QUERY = "(max-width: 768px)";
export const MAX_AGENT_CONVERSATIONS = 40;
export const MAX_AGENT_RUN_ID_BYTES = 256;

export type ConversationDeletionEvent = {
  conversationId: string;
  type: "started" | "deleted" | "settled";
  ledgerChanged?: boolean;
  storageError?: boolean;
};

export const conversationDeletionInFlightIds = new Set<string>();
export const conversationDeletionCompletedIds = new Set<string>();
export const conversationDeletionListeners = new Set<(event: ConversationDeletionEvent) => void>();

export function resetAgentConversationDeletionCoordinatorForTests() {
  conversationDeletionInFlightIds.clear();
  conversationDeletionCompletedIds.clear();
}

export function publishConversationDeletion(event: ConversationDeletionEvent) {
  if (event.type === "started") {
    conversationDeletionInFlightIds.add(event.conversationId);
    conversationDeletionCompletedIds.delete(event.conversationId);
  } else {
    conversationDeletionInFlightIds.delete(event.conversationId);
    if (event.type === "deleted") conversationDeletionCompletedIds.add(event.conversationId);
  }
  for (const listener of conversationDeletionListeners) listener(event);
}
