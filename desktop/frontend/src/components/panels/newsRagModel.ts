// 从 NewsRagPanel.tsx 拆出的纯逻辑；由原文件按需 import。

export const MAX_PDF_FILE_BYTES = 25 * 1024 * 1024;
export const MAX_RESEARCH_PACK_BYTES = 64 * 1024 * 1024;
import { sentimentCivilDate } from "../../lib/sentimentView";
import { ResearchMessage, ResearchOverview } from "../../types/index";

export function researchOperationNotice(result: unknown): {
  tone: "success" | "warning" | "error";
  text: string;
} {
  if (result && typeof result === "object") {
    const record = result as Record<string, unknown>;
    if ("error" in record) {
      return { tone: "error", text: String(record.error) };
    }
    if (record.ocr_required === true) {
      const skippedPages = Array.isArray(record.skipped_pages)
        ? record.skipped_pages.filter((page): page is number => Number.isInteger(page)) : [];
      const extracted = Number(record.extracted_page_count || 0);
      const total = Number(record.page_count || extracted);
      const pages = skippedPages.length ? `第 ${skippedPages.join("、")} 页` : "部分页面";
      return {
        tone: "warning",
        text: `已导入 ${extracted}/${total} 页；${pages}没有可提取文本，需要 OCR 后重新导入。`,
      };
    }
  }
  return { tone: "success", text: "操作已完成，索引状态已刷新。" };
}


export function sourceTierLabel(value?: string): string {
  const labels: Record<string, string> = {
    policy_official: "官方政策",
    news_media: "财经媒体",
    filing: "公告", financial_snapshot: "财务快照", news: "新闻",
    research: "研报", research_report: "研报", community: "社区",
  };
  const key = value ? String(value) : "news";
  return labels[key] || "新闻";
}

export function scopeTypeLabel(value?: string): string {
  return ({ macro: "宏观", industry: "行业", region: "区域", company: "公司" } as Record<string, string>)[value || ""] || value || "政策";
}

export function sentimentClass(value: string): string {
  if (["positive", "bullish", "利好"].includes(value)) return "positive";
  if (["negative", "bearish", "利空"].includes(value)) return "negative";
  return "uncertain";
}

export function groupMessages(messages: ResearchMessage[]) {
  const sorted = messages.map((message, index) => ({ message, index, timestamp: publishTimestamp(message.published_at) }))
    .sort((left, right) => {
      if (left.timestamp === null) return right.timestamp === null ? left.index - right.index : 1;
      if (right.timestamp === null) return -1;
      return right.timestamp - left.timestamp || left.index - right.index;
    });
  const groups = new Map<string, { key: string; label: string; messages: ResearchMessage[] }>();
  for (const { message, timestamp } of sorted) {
    const date = timestamp === null ? null : sentimentCivilDate(timestamp);
    const key = date || "unknown";
    if (!groups.has(key)) groups.set(key, { key, label: date || "日期未提供", messages: [] });
    groups.get(key)!.messages.push(message);
  }
  return [...groups.values()];
}
export function publishTimestamp(value?: string | null): number | null {
  if (!value) return null;
  // The research cache uses China civil time for timestamps without an offset.
  const normalized = /^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(:\d{2}(\.\d+)?)?$/.test(value) ? `${value.replace(" ", "T")}+08:00` : value;
  const timestamp = Date.parse(normalized);
  return Number.isFinite(timestamp) ? timestamp : null;
}
export function isToday(value?: string | null): boolean {
  const timestamp = publishTimestamp(value);
  return timestamp !== null && sentimentCivilDate(timestamp) === sentimentCivilDate(Date.now());
}

export function safeExternalUrl(value?: string | null): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.toString() : null;
  } catch {
    return null;
  }
}

export function formatEpoch(value?: number): string {
  return value ? new Date(value).toLocaleDateString("zh-CN", {
    month: "2-digit", day: "2-digit",
  }) : "刚刚";
}

export function formatResearchUpdatedAt(overview: ResearchOverview | null): string {
  if (!overview) return "";
  const raw = overview.updated_at_epoch_ms || overview.last_refresh_at || overview.last_updated_at;
  if (!raw) return "";
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
}

export function formatScore(value?: number | null): string {
  return Number.isFinite(value) ? Number(value).toFixed(4) : "—";
}

export function validateResearchFile(file: Pick<File, "name" | "size">, endpoint: string): void {
  const pdf = endpoint === "/api/research/import-pdf";
  const limit = pdf ? MAX_PDF_FILE_BYTES : MAX_RESEARCH_PACK_BYTES;
  if (file.size > limit) {
    throw new Error(`${file.name || "文件"} 超过 ${pdf ? "25 MB" : "64 MB"} 上限`);
  }
}

export async function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error || new Error("读取文件失败"));
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const separator = result.indexOf(",");
      if (separator < 0) {
        reject(new Error("文件编码失败"));
        return;
      }
      resolve(result.slice(separator + 1));
    };
    reader.readAsDataURL(file);
  });
}
