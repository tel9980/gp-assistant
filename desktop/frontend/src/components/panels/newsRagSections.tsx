// 从 NewsRagPanel.tsx 拆出的子组件；由原文件按需 import。
import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { BookOpen, ChevronLeft, ChevronRight, Database, Download, ExternalLink, FileText, MessageSquareText, Plus, RefreshCw, RotateCcw, Search, Trash2, Upload, X } from "lucide-react";
import { normalizeNewsGroups } from "../../lib/contracts";
import { formatBytes, formatDateTime, normalizeStockCode } from "../../lib/format";
import { postJson } from "../../lib/tauri";
import { NewsRagResult, ResearchAnswer, ResearchCitation, ResearchIndexStatus, ResearchMessage, ResearchThread, WatchlistItem } from "../../types/index";
import { Sheet } from "../ui/Sheet";
import { fileToBase64, formatEpoch, formatScore, researchOperationNotice, safeExternalUrl, scopeTypeLabel, sentimentClass, sourceTierLabel, validateResearchFile } from "./newsRagModel";
export function ResearchPortal({ children }: { children: ReactNode }) {
  return document.body ? createPortal(children, document.body) : children;
}
export function ResearchSurfaceSheet({active,open,close,label,children}: {active:boolean;open:boolean;close:()=>void;label:string;children:ReactNode}) {
  if (!active) return children;
  return <Sheet open={open} onClose={close} label={label} className="research-surface-sheet" backdropClassName="research-surface-backdrop">{children}</Sheet>;
}
export function EventGroup(props: {
  label: string;
  tone: string;
  messages: ResearchMessage[];
  selectedId: string | null;
  selectEvent: (message: ResearchMessage) => void;
  setEventRef: (id: string, element: HTMLButtonElement | null) => void;
  pushCitation: (citation: ResearchCitation) => void;
}) {
  if (!props.messages.length) return null;
  return <section className={`research-event-group ${props.tone}`}>
    <header><span>{props.label}</span><b>{props.messages.length}</b></header>
    {props.messages.map((message) => {
      const selected = props.selectedId === message.id;
      return <div className={`research-event-shell${selected ? " selected expanded" : ""}`} key={message.id}>
        <button type="button" ref={(element) => props.setEventRef(message.id, element)}
          aria-expanded={selected}
          className={`research-event ${sentimentClass(message.sentiment)}${message.unread ? " unread" : ""}${selected ? " selected" : ""}`}
          onClick={() => props.selectEvent(message)}
          onKeyDown={(event) => {
            if (event.key === "Escape" && selected) {
              event.preventDefault();
              props.selectEvent(message);
            }
          }}>
          <span className={`research-dot research-event-dot-desktop ${message.unread ? "unread" : "neutral"}`} aria-label={message.unread ? "未读" : undefined} />
          <span className="research-event-content">
            <strong>{message.title}</strong>
            <span className="research-event-summary">{message.summary}</span>
            <span className="research-event-meta">
              <span className="research-source-name">{message.source_name}</span>
              <time>{formatDateTime(message.published_at)}</time>
              {message.unread && <span className="research-unread-label">未读</span>}
            </span>
          </span>
        </button>
        {selected && <div className="research-event-detail">
          <p>{message.summary}</p>
          <div className="research-event-detail-meta">
            <span className="research-badge">{sourceTierLabel(message.source_tier)}</span>
            {message.scope_type && <span className="research-badge">{scopeTypeLabel(message.scope_type)}</span>}
            {message.scope_tags?.map((tag) => <span className="research-badge" key={tag}>{tag}</span>)}
          </div>
          {message.citations?.length ? <div className="research-event-detail-evidence">
            <span>已关联 {message.citations.length} 条证据</span>
            <div className="research-event-citation-links">
              {message.citations.map((item) => <button type="button"
                key={item.citation_id} onClick={() => props.pushCitation(item)}>
                查看 {item.citation_id}
              </button>)}
            </div>
          </div> : null}
        </div>}
      </div>;
    })}
  </section>;
}

export function ResearchEmptyState(props: { refreshing: boolean; onRefresh: () => void; code: string; needsWatchlist?: boolean; onGoToScreen?: () => void; onPickStock: () => void; }) {
  return <div className="research-empty"><BookOpen size={28} /><strong>{props.code ? "当前股票暂无消息" : "暂无消息"}</strong>
    <p>{props.code ? "更新后查看这只股票的最新消息。" : props.needsWatchlist ? "先去选股收藏，或搜索股票代码。" : "选择股票后更新消息。"}</p>
    <div className="research-empty-actions"><button type="button" className="research-empty-primary" disabled={props.refreshing}
      onClick={props.code ? props.onRefresh : props.needsWatchlist ? props.onGoToScreen : props.onPickStock}>
      {props.code && <RefreshCw size={15} />}<span>{props.code ? props.refreshing ? "更新中" : "更新" : props.needsWatchlist ? "去选股收藏" : "选择股票"}</span>
    </button></div>
  </div>;
}

export function ResearchBriefCounts(props: {
  positive: number;
  negative: number;
  uncertain: number;
  inline?: boolean;
}) {
  const hasBriefStats = props.positive > 0 || props.negative > 0
    || props.uncertain > 0;
  if (!hasBriefStats) return null;
  return <span className={`research-brief-counts${props.inline ? " research-brief-inline-counts" : ""}`}>
    <span className="research-stat positive"><span>利好</span><strong>{props.positive}</strong></span>
    <span className="research-stat negative"><span>利空</span><strong>{props.negative}</strong></span>
    <span className="research-stat"><span>待核查</span><strong>{props.uncertain}</strong></span>
  </span>;
}

export function InboxPanel(props: {
  code: string;
  setCode: (value: string) => void;
  watchlist: WatchlistItem[];
  unreadByStock: Record<string, number>;
  unread: number;
  open: boolean;
  close: () => void;
}) {
  return <aside className={`research-inbox${props.open ? " mobile-open" : ""}`}>
    <span className="research-sheet-handle" aria-hidden="true" />
    <div className="research-pane-heading">
      <div><strong>自选股票</strong></div>
      <button className="research-icon-button research-mobile-close" type="button"
        aria-label="关闭收件箱" title="关闭" onClick={props.close}><X size={17} /></button>
    </div>
    <button type="button" className={`research-stock-row${!props.code ? " active" : ""}`}
      onClick={() => props.setCode("")}>
      <span><strong>全部消息</strong></span>
      {props.unread > 0 && <b>{props.unread}</b>}
    </button>
    {props.watchlist.length === 0 && <p className="research-inbox-empty-hint">暂无自选股票。</p>}
    {props.watchlist.map((item) => {
      const stockCode = normalizeStockCode(item.code);
      const unread = props.unreadByStock[stockCode] || 0;
      return <div className={`research-stock-row-wrap${props.code === stockCode ? " active" : ""}`} key={stockCode}>
      <button type="button"
        className={`research-stock-row${props.code === stockCode ? " active" : ""}`}
        onClick={() => props.setCode(stockCode)}>
        <span><strong>{item.name || stockCode}</strong><small>{stockCode}</small></span>
        {unread > 0 && <b>{unread}</b>}
      </button>
      </div>;
    })}

  </aside>;
}

export function ResearchThreads(props: {
  threads: ResearchThread[]; threadId: string; setThread: (thread: ResearchThread) => void; createThread: () => void;
  deleteThread: (thread: ResearchThread) => void; deletingThreadId: string | null; deleteCandidateId: string | null; asking: boolean;
}) {
  return     <div className="research-inbox-section">
      <div className="research-pane-heading compact"><span>研究会话</span>
        <button className="research-icon-button" type="button" aria-label="新建研究会话"
          title="新建会话" disabled={props.asking || Boolean(props.deletingThreadId)} onClick={props.createThread}><Plus size={16} /></button>
      </div>
      {props.threads.slice(0, 20).map((thread) => <div key={thread.id}
        className={`research-thread-row${thread.id === props.threadId ? " active" : ""}`}>
        <button type="button" className="research-thread-main"
          disabled={Boolean(props.deletingThreadId)} onClick={() => props.setThread(thread)}>
          <MessageSquareText size={15} aria-hidden="true" />
          <span><strong>{thread.title}</strong><small>{formatEpoch(thread.updated_at_epoch_ms)}</small></span>
        </button>
        <button type="button" className={`research-thread-delete${props.deleteCandidateId === thread.id ? " confirm" : ""}`}
          aria-label={`删除研究会话：${thread.title || "未命名会话"}`}
          title="删除会话" disabled={props.asking || props.deletingThreadId === thread.id}
          onClick={() => props.deleteThread(thread)}>
          {props.deleteCandidateId === thread.id ? "确认删除？" : <Trash2 size={14} aria-hidden="true" />}
        </button>
      </div>)}
    </div>;
}

export function Answers(props: {
  answers: ResearchAnswer[];
  pushCitation: (value: ResearchCitation) => void;
  highlightedId: string | null;
  sectionRef: React.MutableRefObject<HTMLElement | null>;
}) {
  if (!props.answers.length) return null;
  return <section className="research-answers">
    <div className="research-section-heading"><div><span>历史问答</span>
      <small>回答与引用均保存在本机</small></div>
    </div>
    {props.answers.map((answer, index) => <article
      key={answer.id || answer.question + index}
      ref={index === props.answers.length - 1 ? props.sectionRef : undefined}
      className={`research-answer${props.highlightedId === (answer.id || `${answer.question}-${index}`) ? " is-highlighted" : ""}`}>
      <div className="research-question"><span>问</span><p>{answer.question}</p></div>
      <div className="research-answer-body">
        <div className="research-answer-mode">
          {answer.mode === "model" ? "模型综合" : "证据摘录"}
        </div>
        <p><CitationRichText text={answer.answer} citations={answer.citations}
          onCitation={props.pushCitation} /></p>
        <div className="research-citation-chips">
          {answer.citations.map((item) => <button type="button"
            key={item.citation_id + index} onClick={() => props.pushCitation(item)}>
            <b>{item.citation_id}</b><span>{item.title}</span>
          </button>)}
        </div>
        {(answer.model_warning || answer.vector_warning) &&
          <small className="research-fallback-note">
            已自动降级：{answer.model_warning || answer.vector_warning}
          </small>}
      </div>
    </article>)}
  </section>;
}

export function EvidencePanel(props: {
  citation: ResearchCitation | null;
  citationIndex: number;
  citationCount: number;
  onPrevious: () => void;
  onNext: () => void;
  close: () => void;
}) {
  return <aside className={`research-evidence${props.citation ? " has-selection" : ""}`}
    aria-label="引用证据检查器">
    <span className="research-sheet-handle" aria-hidden="true" />
    <div className="research-pane-heading">
      <div><span>证据检查器</span><strong>原文可回溯</strong></div>
      {props.citation && <div className="research-citation-history" aria-label="引用历史">
        <button type="button" aria-label="上一条证据" title="上一条证据"
          disabled={props.citationIndex <= 0} onClick={props.onPrevious}><ChevronLeft size={15} /></button>
        <span>{props.citationIndex + 1} / {props.citationCount}</span>
        <button type="button" aria-label="下一条证据" title="下一条证据"
          disabled={props.citationIndex >= props.citationCount - 1} onClick={props.onNext}><ChevronRight size={15} /></button>
      </div>}
      <button className="research-icon-button research-evidence-close" type="button"
        aria-label="关闭证据检查器" title="关闭" onClick={props.close}><X size={17} /></button>
    </div>
    {props.citation ? <EvidenceInspector citation={props.citation} />
      : <div className="research-evidence-empty">
        <span className="research-evidence-empty-icon"><Search size={18} /></span>
        <div><strong>证据会在这里展开</strong>
          <p>完成一次证据问答，再点击回答中的引用编号查看原文。</p></div>
        <ol aria-label="证据检查步骤">
          <li><span>1</span><div><strong>定位引用</strong><p>先点回答中的引用编号。</p></div></li>
          <li><span>2</span><div><strong>核对原文</strong><p>对照页码、来源与摘录。</p></div></li>
          <li><span>3</span><div><strong>回到原文</strong><p>需要时打开公开链接复核。</p></div></li>
        </ol>
      </div>}
  </aside>;
}

export function EvidenceInspector({ citation: item }: { citation: ResearchCitation }) {
  const externalUrl = safeExternalUrl(item.url);
  return <div className="research-evidence-card">
    <div className="research-citation-ledger"><span>{item.citation_id}</span>
      <div><strong>{sourceTierLabel(item.source_tier)}</strong><small>{item.source_name}</small></div>
    </div>
    <h2>{item.title}</h2>
    {item.unavailable && <p role="status" className="research-community-warning">原证据不可用：以下为回答时保存的历史摘录，未绑定到当前资料。</p>}
    <div className="research-evidence-meta">
      <span>{item.published_at ? formatDateTime(item.published_at) : "日期未提供"}</span>
      {item.page_number != null && <span>第 {item.page_number} 页</span>}
    </div>
    <blockquote>{item.excerpt}</blockquote>
    <details className="research-retrieval-details"><summary>检索技术详情</summary><dl>
      <div><dt>融合分数</dt><dd>{formatScore(item.retrieval_score)}</dd></div>
      <div><dt>关键词</dt><dd>{formatScore(item.lexical_score)}</dd></div>
      <div><dt>向量</dt><dd>{item.vector_score == null ? "未使用" : formatScore(item.vector_score)}</dd></div>
    </dl></details>
    {externalUrl && <a href={externalUrl} target="_blank" rel="noreferrer">
      <ExternalLink size={15} />打开原文
    </a>}
    {item.source_tier === "community" && <p className="research-community-warning">
      社区信息只能作为线索，不能单独支撑事实结论。
    </p>}
  </div>;
}

export function CitationRichText(props: {
  text: string;
  citations: ResearchCitation[];
  onCitation: (citation: ResearchCitation) => void;
}) {
  return <>{props.text.split(/(\[C\d+\])/g).map((part, index) => {
    const match = /^\[(C\d+)\]$/.exec(part);
    const item = match ? props.citations.find((citation) =>
      citation.citation_id === match[1]) : undefined;
    return item ? <button type="button" className="research-inline-citation"
      key={part + index} onClick={() => props.onCitation(item)}>{part}</button>
      : <span key={part + index}>{part}</span>;
  })}</>;
}
export function KnowledgeDrawer(props: {
  code: string;
  mobile: boolean;
  status: ResearchIndexStatus | null;
  management: (action: () => Promise<unknown>) => Promise<void>;
  busy: boolean;
  result: unknown;
  close: () => void;
}) {
  const [url, setUrl] = useState("");
  const [allowRemoteExport, setAllowRemoteExport] = useState(false);
  const inputFile = async (file: File, endpoint: string) => props.management(async () => {
    validateResearchFile(file, endpoint);
    return postJson(endpoint, {
      bytes_base64: await fileToBase64(file),
      stock_codes: props.code ? [props.code] : [],
      ...(endpoint === "/api/research/import-pdf"
        ? { remote_export_allowed: allowRemoteExport }
        : {}),
    });
  });
  const operationNotice = props.result == null ? null : researchOperationNotice(props.result);

  return <Sheet open onClose={props.close} label="知识库管理" className="research-surface-sheet" backdropClassName="research-surface-backdrop">
    <aside className="knowledge-drawer">
      <header><div><span>{props.mobile ? "资料包同步" : "知识库管理"}</span>
        <h2>{props.mobile ? "导入与回滚" : "资料与索引"}</h2></div>
        <button type="button" className="research-icon-button" aria-label="关闭知识库管理"
          title="关闭" onClick={props.close}><X size={18} /></button>
      </header>

      <section className="knowledge-status">
        <div><span>文档</span><strong>{props.status?.document_count || 0}</strong></div>
        <div><span>分块 / FTS</span><strong>
          {props.status?.chunk_count || 0} / {props.status?.fts_count || 0}
        </strong></div>
        <div><span>向量</span><strong>{props.status?.embedding_count || 0}</strong></div>
        <div><span>占用</span><strong>{formatBytes(props.status?.database_bytes)}</strong></div>
      </section>

      {!props.mobile && <section className="knowledge-section">
        <h3>导入资料</h3>
        <label><span>公网 HTTPS 地址</span>
          <input value={url} onChange={(event) => setUrl(event.target.value)}
            placeholder="https://" />
        </label>
        <label className="knowledge-export-consent">
          <input type="checkbox" checked={allowRemoteExport}
            onChange={(event) => setAllowRemoteExport(event.target.checked)} />
          <span>允许把这份资料发送给远程回答模型（默认关闭）</span>
        </label>
        <button type="button" disabled={props.busy || !url.trim()}
          onClick={() => void props.management(() => postJson("/api/research/import-url", {
            url: url.trim(), stock_codes: props.code ? [props.code] : [],
            remote_export_allowed: allowRemoteExport,
          }))}><Download size={15} />导入 URL</button>
        <label className="knowledge-file-button"><FileText size={16} />
          <span>导入文本型 PDF</span>
          <input type="file" accept="application/pdf,.pdf" onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) void inputFile(file, "/api/research/import-pdf");
            event.currentTarget.value = "";
          }} />
        </label>
        <small>扫描件需先 OCR；单文件上限 25 MB、500 页。</small>
      </section>}

      {!props.mobile && <section className="knowledge-section">
        <h3>索引维护</h3>
        <div className="knowledge-button-row">
          <button type="button" disabled={props.busy}
            onClick={() => void props.management(() =>
              postJson("/api/research/rebuild-index", {}))}>
            <RefreshCw size={15} />重建 FTS
          </button>
          <button type="button" disabled={props.busy}
            onClick={() => void props.management(() =>
              postJson("/api/research/rebuild-embeddings", {}))}>
            <Search size={15} />生成向量
          </button>
        </div>
        <p>{!props.status ? "正在读取索引状态。"
          : !(props.status.fts_healthy ?? props.status.healthy) ? "FTS 索引存在缺口，建议重建 FTS。"
          : (props.status?.embedding_pending_count || 0) > 0
            ? `FTS 索引完整；还有 ${props.status?.embedding_pending_count} 个分块等待生成向量。`
            : props.status.integrity_healthy === false
              ? "索引存在完整性问题，建议重建索引。"
            : props.status?.hybrid_ready
              ? "FTS 与向量索引完整，可执行混合检索。"
              : "FTS 索引完整；当前平台使用关键词检索。"}</p>
      </section>}

      <section className="knowledge-section">
        <h3>同步与回滚</h3>
        <div className={`knowledge-button-row${props.mobile ? " single" : ""}`}>
          {!props.mobile && <button type="button" disabled={props.busy}
            onClick={() => void props.management(() =>
              postJson("/api/research/pack/export", {}))}>
            <Upload size={15} />导出 v2 包
          </button>}
          <button type="button" disabled={props.busy}
            onClick={() => void props.management(() =>
              postJson("/api/research/pack/rollback", {}))}>
            <RotateCcw size={15} />回滚导入
          </button>
        </div>
        <label className="knowledge-file-button"><Database size={16} />
          <span>导入 SQLite v2 / 旧版包</span>
          <input type="file" accept=".sqlite,.json" onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) void inputFile(file, "/api/research/pack/import");
            event.currentTarget.value = "";
          }} />
        </label>
      </section>

      {props.busy && <div className="knowledge-operation">
        <RefreshCw size={15} className="is-spinning" />正在处理…
      </div>}
      {!props.busy && operationNotice &&
        <div className={`knowledge-operation ${operationNotice.tone}`}>
          {operationNotice.text}
        </div>}
      <details className="knowledge-diagnostics"><summary>高级诊断</summary>
        <pre>{JSON.stringify(props.status, null, 2)}</pre>
        {props.result != null && <pre>{JSON.stringify(props.result, null, 2)}</pre>}
      </details>
    </aside>
  </Sheet>;
}

export function NewsRagView({ result }: { result: NewsRagResult }) {
  const groups = normalizeNewsGroups(result.sentiment_groups);
  const items = [...groups.positive, ...groups.negative, ...groups.mixed, ...groups.uncertain];
  return <div className="agent-news-evidence">
    <header><strong>消息证据</strong><span>{items.length} 条</span></header>
    {!items.length ? <p>当前没有命中的消息证据。</p>
      : items.slice(0, 12).map((item, index) => <article
        key={(item.title || "消息") + index}>
        <span>{sourceTierLabel(item.source_tier)}</span>
        <div><strong>{item.title || "未命名消息"}</strong><p>{item.summary}</p></div>
      </article>)}
  </div>;
}
