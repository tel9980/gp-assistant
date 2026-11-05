//! Agent（本地规则式问答）相关的意图识别、工具调用、证据拼装与响应组装。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use agent::*;` 保证调用点不变。
use crate::*;
use serde_json::{json, Value};

pub fn agent_skill_registry() -> Vec<AgentSkill> {
    vec![
        agent_skill(
            "stock_snapshot",
            "Stock Snapshot",
            "Local quote, observe and financial snapshot.",
            "desktop,android",
            "local_cache",
            "Return available observe data or explain missing local cache.",
        ),
        agent_skill(
            "stock_news",
            "Stock News",
            "News and RAG evidence for recent positive and negative signals.",
            "desktop,android",
            "rag_or_news_cache",
            "Return short-link news or imported mobile RAG package when full RAG is unavailable.",
        ),
        agent_skill(
            "stock_screen",
            "Stock Screen",
            "Rule based candidate screening over the local universe.",
            "desktop,android",
            "local_cache",
            "Ask for clearer criteria when the intent cannot be mapped.",
        ),
        agent_skill(
            "trend_analysis",
            "Trend Analysis",
            "Technical trend ranking and local OHLC indicators.",
            "desktop,android",
            "local_history",
            "Return skipped-count warnings when OHLC history is incomplete.",
        ),
        agent_skill(
            "sector_analysis",
            "Sector Analysis",
            "Sector and relation driven candidate discovery.",
            "desktop",
            "local_relations",
            "Fall back to ordinary screening when relation data is unavailable.",
        ),
        agent_skill(
            "watchlist_action",
            "Watchlist Action",
            "Local watchlist-only add, remove and review workflow.",
            "desktop,android",
            "local_state",
            "Never place trades; explain local-only scope.",
        ),
        agent_skill(
            "portfolio_simulation",
            "Portfolio Simulation",
            "Watchlist or candidate portfolio observation and backtest.",
            "desktop",
            "local_backtest",
            "Return research-only backtest or desktop-only downgrade notice.",
        ),
    ]
}

pub fn agent_skill(
    key: &str,
    title: &str,
    description: &str,
    platforms: &str,
    evidence_level: &str,
    fallback: &str,
) -> AgentSkill {
    AgentSkill {
        key: key.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        input_schema: json!({ "query": "string", "symbols": ["string"], "mode": "quick|expert|research" }),
        output_schema: json!({ "sections": ["summary", "evidence", "risks"], "research_only": true }),
        platforms: platforms.split(',').map(|item| item.to_string()).collect(),
        evidence_level: evidence_level.to_string(),
        fallback: fallback.to_string(),
    }
}

pub fn agent_finalize_response(
    mut response: AgentResponse,
    message: &str,
    mode: Option<&str>,
) -> AgentResponse {
    let mode_text = mode.unwrap_or("quick").to_string();
    let action = response.action.clone();
    let symbols = extract_codes(message);
    response.intent = Some(AgentIntent {
        kind: agent_intent_kind(&action).to_string(),
        query: message.trim().to_string(),
        symbols,
        window: agent_window_hint(message),
        depth: agent_depth_for_mode(&mode_text).to_string(),
        mode: mode_text,
    });
    response.tool_calls = agent_tool_calls_for_response(&response);
    response.evidence_summary = agent_evidence_for_response(&response);
    response.answer_sections = agent_beginner_sections_for_response(&response);
    response.warnings = agent_warnings_for_response(&response);
    response.next_actions = agent_next_actions_for_response(&response);
    response
}

pub fn agent_intent_kind(action: &str) -> &'static str {
    match action {
        "trend_screen" => "trend_analysis",
        "graph_screen" => "sector_analysis",
        "backtest" => "portfolio_simulation",
        "screen" => "stock_screen",
        "observe_stock" => "stock_snapshot",
        "news_rag" => "stock_news",
        "watchlist_action" => "watchlist_action",
        _ => "clarify",
    }
}
pub fn agent_depth_for_mode(mode: &str) -> &'static str {
    match mode {
        "research" => "research",
        "expert" => "expert",
        _ => "quick",
    }
}

pub fn agent_window_hint(message: &str) -> Option<String> {
    let lower = message.to_lowercase();
    if lower.contains("recent") || message.contains("近期") || message.contains("最近") {
        Some("recent".to_string())
    } else if lower.contains("today") || message.contains("今天") || message.contains("今日") {
        Some("today".to_string())
    } else {
        None
    }
}
pub fn agent_tool_calls_for_response(response: &AgentResponse) -> Vec<AgentToolCall> {
    if response.action == "clarify" {
        return vec![AgentToolCall {
            id: "intent_parser".to_string(),
            tool: "agent_plan_request".to_string(),
            label: "Understand request".to_string(),
            status: "needs_input".to_string(),
            input: json!({ "action": response.action }),
            output_summary: Some("Need a clearer stock research task.".to_string()),
            warnings: Vec::new(),
        }];
    }
    let tool = agent_intent_kind(&response.action);
    vec![
        AgentToolCall {
            id: "intent_parser".to_string(),
            tool: "agent_plan_request".to_string(),
            label: "Understand request".to_string(),
            status: "ok".to_string(),
            input: json!({ "action": response.action }),
            output_summary: Some(format!("Routed to {tool}.")),
            warnings: Vec::new(),
        },
        AgentToolCall {
            id: format!("tool_{tool}"),
            tool: tool.to_string(),
            label: agent_tool_label(tool).to_string(),
            status: if response.data.is_some() {
                "ok"
            } else {
                "degraded"
            }
            .to_string(),
            input: json!({ "action": response.action }),
            output_summary: Some(agent_data_summary(response)),
            warnings: agent_warnings_for_response(response),
        },
        AgentToolCall {
            id: "synthesizer".to_string(),
            tool: "agent_synthesize_response".to_string(),
            label: "Structure answer".to_string(),
            status: "ok".to_string(),
            input: json!({ "research_only": true }),
            output_summary: Some("Prepared sections, evidence and risk boundary.".to_string()),
            warnings: Vec::new(),
        },
    ]
}

pub fn agent_tool_label(tool: &str) -> &'static str {
    match tool {
        "trend_analysis" => "运行趋势筛选",
        "sector_analysis" => "运行自定义板块筛选",
        "portfolio_simulation" => "运行本地组合观察",
        "stock_screen" => "运行本地选股",
        "stock_snapshot" => "生成个股速览",
        "stock_news" => "整理资讯证据",
        "watchlist_action" => "读取本地自选股",
        _ => "运行本地工具",
    }
}
pub fn agent_data_summary(response: &AgentResponse) -> String {
    let Some(data) = response.data.as_ref() else {
        return "未返回结构化数据。".to_string();
    };
    let returned = data
        .get("returned")
        .and_then(Value::as_u64)
        .or_else(|| {
            data.get("items")
                .and_then(Value::as_array)
                .map(|items| items.len() as u64)
        })
        .or_else(|| {
            data.get("equity_curve")
                .and_then(Value::as_array)
                .map(|items| items.len() as u64)
        });
    match returned {
        Some(count) => format!("返回 {count} 条本地记录。"),
        None => "已返回本地结构化数据。".to_string(),
    }
}
pub fn agent_evidence_for_response(response: &AgentResponse) -> Vec<AgentEvidenceItem> {
    let source = match response.action.as_str() {
        "trend_screen" => "本地 K 线与趋势指标",
        "graph_screen" => "本地自定义筛选兼容结果",
        "backtest" => "本地回测引擎",
        "screen" => "本地股票池缓存",
        "observe_stock" => "本地行情与观察数据",
        "news_rag" => "本地新闻/RAG 降级结果",
        "watchlist_action" => "本地自选股观察池",
        _ => "本地 Agent 路由",
    };
    vec![AgentEvidenceItem {
        title: agent_tool_label(agent_intent_kind(&response.action)).to_string(),
        source: source.to_string(),
        level: if response.data.is_some() {
            "primary"
        } else {
            "degraded"
        }
        .to_string(),
        summary: agent_data_summary(response),
    }]
}
pub fn agent_beginner_sections_for_response(response: &AgentResponse) -> Vec<AgentAnswerSection> {
    if response.action == "observe_stock" {
        if let Some(sections) = agent_beginner_stock_sections(response) {
            return sections;
        }
    }
    agent_sections_for_response(response)
}

pub fn agent_sections_for_response(response: &AgentResponse) -> Vec<AgentAnswerSection> {
    vec![
        AgentAnswerSection {
            title: "结论概览".to_string(),
            bullets: vec![
                response.reply.clone(),
                "仅供选股研究，不构成投资建议。".to_string(),
            ],
        },
        AgentAnswerSection {
            title: "证据摘要".to_string(),
            bullets: agent_evidence_for_response(response)
                .into_iter()
                .map(|item| item.summary)
                .collect(),
        },
        AgentAnswerSection {
            title: "风险边界".to_string(),
            bullets: vec!["不输出买入、卖出、目标价、仓位或收益承诺。".to_string()],
        },
    ]
}
pub fn agent_beginner_stock_sections(response: &AgentResponse) -> Option<Vec<AgentAnswerSection>> {
    let data = response.data.as_ref()?;
    let stock = data.get("stock")?;
    let name = json_string(stock, "name").unwrap_or_else(|| "这只股票".to_string());
    let code = json_string(stock, "code").unwrap_or_default();
    let price = json_f64(stock, "price").map(format_price_text);
    let change = json_f64(stock, "change_pct").map(format_signed_percent_text);
    let pe = json_f64(stock, "pe").map(format_number);
    let pb = json_f64(stock, "pb").map(format_number);
    let roe = json_f64(stock, "roe").map(format_percent);
    let market_cap =
        json_f64(stock, "market_cap_billion").map(|value| format!("{}亿", format_number(value)));

    let trend_signal = data.get("trend").and_then(|trend| trend.get("signal"));
    let status = trend_signal.and_then(|signal| json_string(signal, "status"));
    let signal_type = trend_signal.and_then(|signal| json_string(signal, "signal_type"));
    let swl = trend_signal.and_then(|signal| json_f64(signal, "swl"));
    let sws = trend_signal.and_then(|signal| json_f64(signal, "sws"));
    let support = trend_signal.and_then(|signal| json_f64(signal, "support"));
    let resistance = trend_signal.and_then(|signal| json_f64(signal, "resistance"));
    let breakout = trend_signal.and_then(|signal| json_f64(signal, "breakout"));
    let reversal = trend_signal.and_then(|signal| json_f64(signal, "reversal"));
    let k = trend_signal.and_then(|signal| json_f64(signal, "k"));
    let d = trend_signal.and_then(|signal| json_f64(signal, "d"));
    let j = trend_signal.and_then(|signal| json_f64(signal, "j"));
    let golden_cross = trend_signal
        .and_then(|signal| signal.get("golden_cross"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let dead_cross = trend_signal
        .and_then(|signal| signal.get("dead_cross"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let quant_score = trend_signal.and_then(|signal| json_i64(signal, "quant_score"));
    let quant_score_max = trend_signal
        .and_then(|signal| json_i64(signal, "quant_score_max"))
        .unwrap_or(90);
    let risk_flags = trend_signal
        .and_then(|signal| signal.get("risk_flags"))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let capital = data.get("capital_evidence");
    let capital_score = capital.and_then(|value| json_f64(value, "composite_score"));
    let capital_confidence = capital.and_then(|value| json_string(value, "confidence"));
    let institution_summary = agent_capital_item_summary(capital, "institution_seat");
    let fund_summary = agent_capital_item_summary(capital, "fund_flow");
    let technical_summary = agent_capital_item_summary(capital, "technical_inference");

    let mut conclusion = Vec::new();
    let quote = [
        price.map(|value| format!("最新价 {value}")),
        change.map(|value| format!("涨跌幅 {value}")),
        pe.map(|value| format!("市盈率 {value}")),
        pb.map(|value| format!("市净率 {value}")),
        roe.map(|value| format!("ROE {value}")),
        market_cap.map(|value| format!("总市值 {value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("，");
    if quote.is_empty() {
        conclusion.push(format!(
            "{}{}：已找到本地个股数据，但行情字段不完整。",
            name,
            code_suffix(&code)
        ));
    } else {
        conclusion.push(format!(
            "{}{}：{}。这些是先看公司价格、估值和盈利质量的基础信息。",
            name,
            code_suffix(&code),
            quote
        ));
    }
    conclusion.push(agent_beginner_trend_sentence(
        status.as_deref(),
        signal_type.as_deref(),
        swl,
        sws,
        quant_score,
        quant_score_max,
    ));
    conclusion.push("简单理解：现在更像是“可继续观察”的状态，不等于马上买入；新手应先看趋势有没有延续，再看跌破哪个位置说明判断失效。".to_string());

    let mut evidence = Vec::new();
    if let Some(score) = capital_score {
        evidence.push(format!("资金证据综合分 {}，置信度{}。分数越高，说明本地资金、机构、技术证据越一致；50 分附近代表偏中性，不能单独当买入理由。", format_number(score), capital_confidence.unwrap_or_else(|| "未知".to_string())));
    } else {
        evidence.push("资金证据暂不完整，所以不要只根据这一轮 Agent 结果判断强弱。".to_string());
    }
    if let Some(text) = institution_summary {
        evidence.push(format!("机构席位：{text}。新手可以把它理解为“大资金是否留下过明显痕迹”的线索，但它不是实时买卖信号。"));
    }
    if let Some(text) = fund_summary {
        evidence.push(format!(
            "量价资金：{text}。这是用本地成交量和价格估算的资金线索，可靠性低于真实资金流数据。"
        ));
    }
    if let Some(text) = technical_summary {
        evidence.push(format!(
            "技术承接：{text}。意思是价格趋势和指标是否互相支持。"
        ));
    }
    if evidence.len() == 1 {
        evidence.push(agent_data_summary(response));
    }

    let mut levels = Vec::new();
    if let (Some(support), Some(resistance)) = (support, resistance) {
        levels.push(format!("支撑位约 {}、压力位约 {}。支撑位可以理解为“跌到这里附近要小心是否撑不住”，压力位可以理解为“涨到这里附近可能遇到卖压”。", format_price_text(support), format_price_text(resistance)));
    }
    if let Some(breakout) = breakout {
        levels.push(format!(
            "突破观察位约 {}：如果放量站上，才说明上攻更有说服力。",
            format_price_text(breakout)
        ));
    }
    if let Some(reversal) = reversal {
        levels.push(format!(
            "风险观察位约 {}：如果跌破，说明原来的上升判断可能失效。",
            format_price_text(reversal)
        ));
    }
    if let (Some(k), Some(d), Some(j)) = (k, d, j) {
        let cross = if golden_cross {
            "当前出现 KDJ 金叉，短线动能转强的信号更明显。"
        } else if dead_cross {
            "当前出现 KDJ 死叉，短线动能转弱，需要更谨慎。"
        } else {
            "当前没有明确 KDJ 金叉或死叉，说明短线拐点信号还不够明确。"
        };
        levels.push(format!(
            "KDJ：K={}、D={}、J={}。{cross}",
            format_number(k),
            format_number(d),
            format_number(j)
        ));
    }
    if levels.is_empty() {
        levels.push("关键价位和指标暂不完整，建议先补足历史行情后再判断趋势。".to_string());
    }

    let mut risks = Vec::new();
    if risk_flags.is_empty() {
        risks.push("本地指标没有给出明确风险旗标，但这不代表没有风险；它只代表当前规则没有识别到特别异常。".to_string());
    } else {
        risks.push(format!(
            "本地风险旗标：{}。出现风险旗标时，新手应降低仓促决策的倾向。",
            risk_flags.join("、")
        ));
    }
    risks.push("不要把“趋势较好”理解成“必涨”。股价会受大盘、行业、公告、业绩和情绪影响，任何单一指标都可能失效。".to_string());
    risks.push("这不是投资建议，不输出买入、卖出、目标价、仓位或收益承诺。".to_string());

    Some(vec![
        AgentAnswerSection {
            title: "一句话结论".to_string(),
            bullets: conclusion,
        },
        AgentAnswerSection {
            title: "为什么这么判断".to_string(),
            bullets: evidence,
        },
        AgentAnswerSection {
            title: "新手要盯的价位".to_string(),
            bullets: levels,
        },
        AgentAnswerSection {
            title: "风险和下一步".to_string(),
            bullets: risks,
        },
    ])
}

pub fn agent_beginner_trend_sentence(
    status: Option<&str>,
    signal_type: Option<&str>,
    swl: Option<f64>,
    sws: Option<f64>,
    quant_score: Option<i64>,
    quant_score_max: i64,
) -> String {
    let status_text = match status.unwrap_or_default() {
        "uptrend" => "趋势偏上升",
        "downtrend" => "趋势偏走弱",
        "sideways" => "趋势偏横盘",
        _ => "趋势状态不明确",
    };
    let signal_text = match signal_type.unwrap_or_default() {
        "trend_continuation" => "更像是原有趋势还在延续",
        "breakout" => "更像是尝试突破",
        "reversal" => "更像是可能反转",
        "pullback" => "更像是上涨后的回落观察",
        _ => "暂时没有特别清晰的信号类型",
    };
    let line_text = match (swl, sws) {
        (Some(swl), Some(sws)) if swl > sws => format!(
            "SWL 高于 SWS（{} > {}），可以粗略理解为短中期趋势线偏强。",
            format_price_text(swl),
            format_price_text(sws)
        ),
        (Some(swl), Some(sws)) if swl < sws => format!(
            "SWL 低于 SWS（{} < {}），可以粗略理解为趋势线偏弱。",
            format_price_text(swl),
            format_price_text(sws)
        ),
        (Some(swl), Some(sws)) => format!(
            "SWL 和 SWS 接近（{} / {}），说明趋势强弱还不明显。",
            format_price_text(swl),
            format_price_text(sws)
        ),
        _ => "SWL/SWS 趋势线数据不完整。".to_string(),
    };
    let score_text = quant_score
        .map(|score| format!("本地量化分 {score}/{quant_score_max}。"))
        .unwrap_or_else(|| "本地量化分暂缺。".to_string());
    format!("趋势：{status_text}，{signal_text}。{line_text}{score_text}")
}

pub fn agent_capital_item_summary(capital: Option<&Value>, category: &str) -> Option<String> {
    let item = capital?
        .get("items")?
        .as_array()?
        .iter()
        .find(|item| json_string(item, "category").as_deref() == Some(category))?;
    if let Some(note) = json_string(item, "note").filter(|text| !text.trim().is_empty()) {
        return Some(note);
    }
    if let Some(score) = json_f64(item, "score") {
        return Some(format!(
            "得分 {}，方向 {}",
            format_number(score),
            json_string(item, "sentiment").unwrap_or_else(|| "未知".to_string())
        ));
    }
    json_string(item, "title")
}

pub fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

pub fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

pub fn code_suffix(code: &str) -> String {
    if code.is_empty() {
        String::new()
    } else {
        format!("（{code}）")
    }
}

pub fn format_price_text(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

pub fn format_signed_percent_text(value: f64) -> String {
    match ratio_percentage_points(value) {
        Some(percent) => format!("{percent:+.2}%"),
        None => "—".to_string(),
    }
}
pub fn agent_warnings_for_response(response: &AgentResponse) -> Vec<String> {
    let mut warnings = vec!["仅供选股研究，不构成投资建议。".to_string()];
    if response.action == "backtest" {
        warnings.push("组合观察只使用本地自选股/候选股数据，不是真实交易。".to_string());
    }
    if response.action == "news_rag" {
        warnings.push("当前新闻能力可能使用本地缓存或降级结果，需复核原始来源。".to_string());
    }
    if response.data.is_none() {
        warnings.push("本次请求没有可用的本地证据载荷。".to_string());
    }
    warnings
}
pub fn agent_next_actions_for_response(response: &AgentResponse) -> Vec<String> {
    match response.action.as_str() {
        "trend_screen" => vec![
            "查看候选股详情".to_string(),
            "运行本地回测".to_string(),
            "加入自选股观察".to_string(),
        ],
        "graph_screen" => vec![
            "调整自定义筛选条件".to_string(),
            "比较板块趋势强度".to_string(),
        ],
        "backtest" => vec!["查看回撤与换手".to_string(), "调整回测时间窗口".to_string()],
        "screen" => vec!["收紧筛选条件".to_string(), "继续做趋势分析".to_string()],
        "observe_stock" => vec!["查看趋势指标".to_string(), "补充资讯证据".to_string()],
        "news_rag" => vec!["核验原始来源".to_string(), "继续看财务和资金面".to_string()],
        "watchlist_action" => vec![
            "用自选股做组合观察".to_string(),
            "对自选股运行回测".to_string(),
        ],
        _ => vec!["补充股票代码、板块或筛选条件".to_string()],
    }
}
pub fn run_agent_with_mock(message: &str) -> CoreResult<AgentResponse> {
    run_agent_with_source(&MockDataSource, message, None)
}

pub fn run_agent_with_data(data: &CoreDataSet, message: &str) -> CoreResult<AgentResponse> {
    run_agent_with_data_and_context(data, message, &AgentContext::default())
}

pub fn run_agent_with_data_and_context(
    data: &CoreDataSet,
    message: &str,
    context: &AgentContext,
) -> CoreResult<AgentResponse> {
    let source = StaticDataSource::new(data);
    run_agent_with_source_and_context(&source, message, None, context)
}

pub fn run_agent_stream_with_data_events(
    data: &CoreDataSet,
    message: &str,
    run_id: Option<&str>,
    mode: Option<&str>,
) -> Vec<AgentStreamEvent> {
    run_agent_stream_with_data_events_with_context(
        data,
        message,
        run_id,
        mode,
        &AgentContext::default(),
    )
}

pub fn run_agent_stream_with_data_events_with_context(
    data: &CoreDataSet,
    message: &str,
    run_id: Option<&str>,
    mode: Option<&str>,
    context: &AgentContext,
) -> Vec<AgentStreamEvent> {
    let source = StaticDataSource::new(data);
    run_agent_stream_with_source_events_with_context(&source, message, run_id, mode, context)
}

pub fn run_agent_with_source(
    source: &impl MarketDataSource,
    message: &str,
    mode: Option<&str>,
) -> CoreResult<AgentResponse> {
    run_agent_with_source_and_context(source, message, mode, &AgentContext::default())
}

pub fn run_agent_with_source_and_context(
    source: &impl MarketDataSource,
    message: &str,
    mode: Option<&str>,
    context: &AgentContext,
) -> CoreResult<AgentResponse> {
    let lower = message.to_lowercase();
    let criteria = heuristic_criteria(message);
    let codes = extract_codes(message);
    let watchlist_codes = agent_watchlist_codes(context);
    let wants_watchlist = contains_any(&lower, &["自选", "自选股", "观察池", "watchlist"]);
    let wants_portfolio = contains_any(
        &lower,
        &[
            "组合",
            "模拟组合",
            "组合观察",
            "回测",
            "backtest",
            "收益曲线",
        ],
    );

    if wants_watchlist && !wants_portfolio {
        let data = agent_watchlist_data(context);
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已读取本地自选股观察池。"),
                action: "watchlist_action".to_string(),
                criteria: None,
                backtest: None,
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if wants_portfolio {
        let default_end_date = current_system_date_yyyymmdd();
        let use_watchlist = wants_watchlist || !watchlist_codes.is_empty();
        let backtest = BacktestRequest {
            algorithm_version: None,
            parameter_set_id: None,
            scoring_parameters: None,
            criteria,
            source: if use_watchlist {
                "watchlist".to_string()
            } else {
                default_backtest_source()
            },
            strategy_mode: default_backtest_strategy_mode(),
            stock_codes: if use_watchlist {
                watchlist_codes
            } else {
                Vec::new()
            },
            start_date: extract_date(message, "20200101", true),
            end_date: extract_date(message, &default_end_date, false),
            top_n: default_top_n(),
            initial_cash: default_initial_cash(),
            rebalance_frequency: default_rebalance_frequency(),
            transaction_cost_bps: default_transaction_cost_bps(),
            benchmark: default_backtest_benchmark(),
        };
        let data = serde_json::to_value(backtest_with_source(source, &backtest)?)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已基于本地数据完成组合观察/回测。"),
                action: "backtest".to_string(),
                criteria: None,
                backtest: Some(backtest),
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if contains_any(
        &lower,
        &[
            "利好", "利空", "新闻", "资讯", "消息", "公告", "舆情", "news", "rag",
        ],
    ) {
        let data = agent_news_data(source, message, &codes)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已整理本地可用的资讯线索与风险边界。"),
                action: "news_rag".to_string(),
                criteria: None,
                backtest: None,
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if contains_any(
        &lower,
        &[
            "财务",
            "资金",
            "资金面",
            "个股",
            "速览",
            "分析",
            "看一下",
            "观察",
            "snapshot",
            "observe",
        ],
    ) && !codes.is_empty()
    {
        let code = codes.first().cloned().unwrap_or_default();
        let observe_request = StockObserveRequest {
            code,
            start_date: extract_date(message, &default_start_date(), true),
            end_date: extract_date(message, &default_end_date(), false),
            series_limit: default_series_limit(),
            include_order_book: false,
        };
        let data = serde_json::to_value(observe_with_source(source, &observe_request)?)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已生成个股本地观察摘要。"),
                action: "observe_stock".to_string(),
                criteria: None,
                backtest: None,
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if contains_any(
        &lower,
        &[
            "趋势",
            "趋势股",
            "上升趋势",
            "技术",
            "量价",
            "短线",
            "主力",
            "支撑",
            "阻力",
            "swl",
            "sws",
            "trend",
        ],
    ) {
        let default_end_date = current_system_date_yyyymmdd();
        let trend_request = TrendScreenRequest {
            criteria,
            start_date: extract_date(message, "20200101", true),
            end_date: extract_date(message, &default_end_date, false),
            limit: 10,
        };
        let data = serde_json::to_value(trend_screen_with_source(source, &trend_request)?)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已按趋势指标完成候选股排序。"),
                action: "trend_screen".to_string(),
                criteria: None,
                backtest: None,
                graph_screen: None,
                trend_screen: Some(trend_request),
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if contains_any(
        &lower,
        &[
            "板块",
            "行业",
            "主题",
            "概念",
            "半导体",
            "新能源",
            "白酒",
            "银行",
            "sector",
        ],
    ) {
        let data = serde_json::to_value(screen_with_source(source, &criteria)?)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已按板块/主题条件完成本地候选筛选。"),
                action: "screen".to_string(),
                criteria: Some(criteria),
                backtest: None,
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    if contains_any(
        &lower,
        &["选股", "筛选", "挑股", "候选", "roe", "pe", "pb", "screen"],
    ) {
        let data = serde_json::to_value(screen_with_source(source, &criteria)?)?;
        return Ok(agent_finalize_response(
            AgentResponse {
                intent: None,
                tool_calls: Vec::new(),
                evidence_summary: Vec::new(),
                answer_sections: Vec::new(),
                warnings: Vec::new(),
                next_actions: Vec::new(),
                reply: mode_reply(mode, "已按描述筛选候选股票。"),
                action: "screen".to_string(),
                criteria: Some(criteria),
                backtest: None,
                graph_screen: None,
                trend_screen: None,
                data: Some(data),
            },
            message,
            mode,
        ));
    }

    Ok(agent_finalize_response(
        AgentResponse {
            intent: None,
            tool_calls: Vec::new(),
            evidence_summary: Vec::new(),
            answer_sections: Vec::new(),
            warnings: Vec::new(),
            next_actions: Vec::new(),
            reply: mode_reply(
                mode,
                "请说明要看个股、资讯、趋势筛选、自选股组合观察，还是普通选股。",
            ),
            action: "clarify".to_string(),
            criteria: None,
            backtest: None,
            graph_screen: None,
            trend_screen: None,
            data: None,
        },
        message,
        mode,
    ))
}
pub fn run_agent_stream_with_source_events(
    source: &impl MarketDataSource,
    message: &str,
    run_id: Option<&str>,
    mode: Option<&str>,
) -> Vec<AgentStreamEvent> {
    run_agent_stream_with_source_events_with_context(
        source,
        message,
        run_id,
        mode,
        &AgentContext::default(),
    )
}

pub fn run_agent_stream_with_source_events_with_context(
    source: &impl MarketDataSource,
    message: &str,
    run_id: Option<&str>,
    mode: Option<&str>,
    context: &AgentContext,
) -> Vec<AgentStreamEvent> {
    let run_id = run_id.unwrap_or("gp-agent-run").to_string();
    let mut events = vec![
        agent_status_event(&run_id, "understand", "理解任务", 8, None),
        agent_status_event(&run_id, "intent", "选择工具", 24, None),
        agent_status_event(&run_id, "execute", "执行本地工具", 64, None),
    ];

    match run_agent_with_source_and_context(source, message, mode, context) {
        Ok(response) => {
            let action = response.action.clone();
            for call in response.tool_calls.clone() {
                events.push(agent_payload_event(
                    &run_id,
                    "tool_start",
                    Some(action.as_str()),
                    json!({
                        "id": call.id,
                        "tool": call.tool,
                        "label": call.label,
                        "input": call.input,
                    }),
                ));
                events.push(agent_payload_event(
                    &run_id,
                    "tool_result",
                    Some(action.as_str()),
                    json!({
                        "id": call.id,
                        "tool": call.tool,
                        "status": call.status,
                        "output_summary": call.output_summary,
                        "warnings": call.warnings,
                    }),
                ));
            }
            if !response.evidence_summary.is_empty() {
                events.push(agent_payload_event(
                    &run_id,
                    "evidence",
                    Some(action.as_str()),
                    json!({ "items": response.evidence_summary.clone() }),
                ));
            }
            events.push(agent_status_event(
                &run_id,
                "format",
                "整理结果",
                88,
                Some(action.as_str()),
            ));
            events.push(agent_status_event(
                &run_id,
                "complete",
                "完成",
                100,
                Some(action.as_str()),
            ));
            events.push(agent_payload_event(
                &run_id,
                "final",
                Some(action.as_str()),
                json!({
                    "intent": response.intent.clone(),
                    "warnings": response.warnings.clone(),
                    "next_actions": response.next_actions.clone(),
                }),
            ));
            events.push(AgentStreamEvent {
                run_id,
                event_type: "result".to_string(),
                stage: None,
                label: None,
                percent: None,
                action: Some(action),
                response: Some(response),
                payload: None,
                message: None,
            });
        }
        Err(error) => {
            events.push(AgentStreamEvent {
                run_id,
                event_type: "error".to_string(),
                stage: None,
                label: None,
                percent: None,
                action: None,
                response: None,
                payload: None,
                message: Some(error.to_string()),
            });
        }
    }

    events
}

pub fn agent_status_event(
    run_id: &str,
    stage: &str,
    label: &str,
    percent: u8,
    action: Option<&str>,
) -> AgentStreamEvent {
    AgentStreamEvent {
        run_id: run_id.to_string(),
        event_type: "status".to_string(),
        stage: Some(stage.to_string()),
        label: Some(label.to_string()),
        percent: Some(percent),
        action: action.map(ToOwned::to_owned),
        response: None,
        payload: None,
        message: None,
    }
}

pub fn agent_payload_event(
    run_id: &str,
    event_type: &str,
    action: Option<&str>,
    payload: Value,
) -> AgentStreamEvent {
    AgentStreamEvent {
        run_id: run_id.to_string(),
        event_type: event_type.to_string(),
        stage: None,
        label: None,
        percent: None,
        action: action.map(ToOwned::to_owned),
        response: None,
        payload: Some(payload),
        message: None,
    }
}

pub fn run_mobile_stock_skill(request: &MobileStockSkillRequest) -> MobileStockSkillResult {
    let mut positive_factors = Vec::new();
    let mut negative_factors = Vec::new();
    let mut neutral_information = Vec::new();
    let mut unverified_leads = Vec::new();
    let mut notes = vec![
        "手机端股票分析 Skill 已按结构化信源生成结论。".to_string(),
        "仅供选股研究，不构成投资建议。".to_string(),
    ];

    for source in request.sources.iter().take(80) {
        let finding = mobile_source_to_finding(source);
        match finding.label.as_str() {
            "positive" => positive_factors.push(finding),
            "negative" => negative_factors.push(finding),
            "neutral" => neutral_information.push(finding),
            _ => unverified_leads.push(finding),
        }
    }

    sort_findings(&mut positive_factors);
    sort_findings(&mut negative_factors);
    sort_findings(&mut neutral_information);
    sort_findings(&mut unverified_leads);

    if request.sources.is_empty() {
        notes.push("未找到可靠信源；请先接入 CNINFO、通达信 F10 或公开新闻 URL。".to_string());
    }
    if request
        .sources
        .iter()
        .any(|item| normalize_source_tier(&item.source_tier) == "community")
    {
        notes.push("社区来源只作为待验证线索，不能单独形成利好或利空事实。".to_string());
    }

    let overview = MobileStockSkillOverview {
        stock_code: request.stock_code.trim().to_uppercase(),
        stock_name: request.stock_name.trim().to_string(),
        overall_label: mobile_overall_label(
            positive_factors.len(),
            negative_factors.len(),
            unverified_leads.len(),
        ),
        summary: mobile_overview_summary(
            request,
            positive_factors.len(),
            negative_factors.len(),
            neutral_information.len(),
            unverified_leads.len(),
        ),
        positive_count: positive_factors.len(),
        negative_count: negative_factors.len(),
        neutral_count: neutral_information.len(),
        unverified_count: unverified_leads.len(),
    };

    MobileStockSkillResult {
        overview,
        positive_factors,
        negative_factors,
        neutral_information,
        unverified_leads,
        notes,
    }
}
pub fn agent_watchlist_codes(context: &AgentContext) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut codes = Vec::new();
    for item in &context.watchlist {
        let code = normalize_stock_code(&item.code);
        if !code.is_empty() && seen.insert(code.clone()) {
            codes.push(code);
        }
    }
    codes
}

pub fn agent_watchlist_data(context: &AgentContext) -> Value {
    let codes = agent_watchlist_codes(context);
    let items = context
        .watchlist
        .iter()
        .map(|item| {
            json!({
                "code": normalize_stock_code(&item.code),
                "name": item.name,
                "industry": item.industry,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "source": "local_watchlist",
        "total": items.len(),
        "returned": items.len(),
        "codes": codes,
        "items": items,
        "notes": ["只读取本地自选股观察池，不执行真实交易。"],
    })
}

pub fn agent_news_data(
    source: &impl MarketDataSource,
    message: &str,
    codes: &[String],
) -> CoreResult<Value> {
    let universe = source.stocks()?;
    let mut items = Vec::new();
    for code in codes.iter().take(5) {
        let stock = universe
            .iter()
            .find(|item| normalize_stock_code(&item.code) == normalize_stock_code(code));
        items.push(json!({
            "code": code,
            "name": stock.map(|item| item.name.clone()).unwrap_or_default(),
            "source": "local_cache",
            "level": "degraded",
            "summary": "当前核心层未接入外部新闻抓取；请结合桌面新闻 RAG 或导入的移动 RAG 包复核。",
        }));
    }
    if items.is_empty() {
        items.push(json!({
            "source": "local_cache",
            "level": "degraded",
            "summary": "未识别到股票代码，已保留为待验证资讯研究问题。",
        }));
    }
    Ok(json!({
        "query": message,
        "source": "local_agent_news_stub",
        "returned": items.len(),
        "items": items,
        "notes": ["社区或未验证来源只作为待验证线索；不输出买卖、目标价或收益承诺。"],
    }))
}
