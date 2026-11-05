//! 移动端 RAG 摘要：来源分级、置信度、发现摘要与整体结论文案。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use mobile::*;` 保证调用点不变。
use crate::*;

pub fn classify_mobile_source(
    source_tier: &str,
    title: &str,
    summary: &str,
    evidence: &str,
) -> String {
    if source_tier == "community" || evidence.trim().is_empty() {
        return "unverified".to_string();
    }
    let text = format!("{title} {summary} {evidence}").to_lowercase();
    let positive = contains_any(
        &text,
        &[
            "增长", "预增", "扭亏", "订单", "中标", "签订", "扩产", "回购", "增持", "分红", "盈利",
            "改善", "突破", "获批", "投产", "景气", "涨价", "positive", "beat", "upgrade",
        ],
    );
    let negative = contains_any(
        &text,
        &[
            "下滑",
            "下降",
            "亏损",
            "预亏",
            "减持",
            "处罚",
            "调查",
            "诉讼",
            "仲裁",
            "违约",
            "终止",
            "取消",
            "风险",
            "计提",
            "减值",
            "停产",
            "限产",
            "退市",
            "negative",
            "miss",
            "downgrade",
        ],
    );

    match (positive, negative) {
        (true, false) => "positive".to_string(),
        (false, true) => "negative".to_string(),
        (true, true) => "neutral".to_string(),
        (false, false) => "neutral".to_string(),
    }
}

pub fn mobile_confidence(source_tier: &str, label: &str, has_evidence: bool) -> f64 {
    if !has_evidence || label == "unverified" {
        return 0.2;
    }
    let base = match source_tier {
        "filing" => 0.9,
        "financial_snapshot" => 0.75,
        "news" => 0.62,
        "research" => 0.55,
        "manual_url" => 0.5,
        "community" => 0.2,
        _ => 0.4,
    };
    round4(base)
}

pub fn mobile_finding_summary(label: &str, source: &MobileStockSourceItem) -> String {
    let body = first_non_empty(&[
        source.summary.as_str(),
        source.evidence.as_str(),
        source.title.as_str(),
    ]);
    let prefix = match label {
        "positive" => "选股研究视角下偏利好",
        "negative" => "选股研究视角下偏利空",
        "neutral" => "中性信息",
        _ => "待验证线索",
    };
    format!("{prefix}: {}", truncate_chars(&body, 120))
}

pub fn mobile_risk_note(label: &str) -> String {
    let note = match label {
        "positive" => "需要用后续公告、财报和成交数据验证，不能据此作交易决定。",
        "negative" => "需要确认事项影响范围和持续性，不能单独作为交易依据。",
        "neutral" => "属于信息披露或背景材料，需结合财务和行情继续验证。",
        _ => "来源不足或未被官方披露确认，只能作为待验证线索。",
    };
    format!("{note} 仅供选股研究，不构成投资建议。")
}

pub fn mobile_overall_label(positive: usize, negative: usize, unverified: usize) -> String {
    if positive == 0 && negative == 0 && unverified > 0 {
        return "unverified".to_string();
    }
    if positive > negative {
        "positive".to_string()
    } else if negative > positive {
        "negative".to_string()
    } else {
        "neutral".to_string()
    }
}

pub fn mobile_overview_summary(
    request: &MobileStockSkillRequest,
    positive: usize,
    negative: usize,
    neutral: usize,
    unverified: usize,
) -> String {
    let target = if request.stock_name.trim().is_empty() {
        request.stock_code.trim()
    } else {
        request.stock_name.trim()
    };
    if positive + negative + neutral + unverified == 0 {
        return format!(
            "{target} 未找到可靠信源，暂不能形成利好利空判断。仅供选股研究，不构成投资建议。"
        );
    }
    format!(
        "{target} 当前命中利好 {positive} 条、利空 {negative} 条、中性 {neutral} 条、待验证 {unverified} 条。仅供选股研究，不构成投资建议。"
    )
}
pub fn sort_findings(findings: &mut [MobileStockSkillFinding]) {
    findings.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| right.published_at.cmp(&left.published_at))
    });
}

pub fn normalize_source_tier(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "filing" | "financial_snapshot" | "news" | "research" | "community" | "manual_url" => {
            normalized
        }
        "tdx" | "f10" => "financial_snapshot".to_string(),
        "cninfo" | "notice" | "announcement" => "filing".to_string(),
        "" => "manual_url".to_string(),
        _ => normalized,
    }
}

pub fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .find_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_default()
}

pub fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            result.push_str("...");
            break;
        }
        result.push(ch);
    }
    result
}

pub fn screen_stocks(universe: &[StockItem], criteria: &ScreenCriteria) -> ScreenResult {
    let mut screened = Vec::new();
    let notes = deducted_profit_rule_notes(universe, criteria);

    for stock in universe {
        let Some(reasons) = matches_stock(stock, criteria) else {
            continue;
        };
        screened.push(score_stock(stock, &reasons, &criteria.score_profile));
    }

    apply_industry_relative_scores(&mut screened, &criteria.score_profile);
    sort_screened(&mut screened, criteria);

    let limit = criteria.limit.clamp(1, 200);
    let (items, promoted) = primary_screen_items(&screened, criteria, limit);
    let groups = screen_result_groups(&screened);
    let mut notes = notes;
    if promoted {
        notes.push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
    }
    notes.push(format!(
        "普通筛选已拆成热门股和综合分大于 {} 的潜力股，每类最多 10 只。",
        POTENTIAL_SCORE_THRESHOLD
    ));
    ScreenResult {
        total: screened.len(),
        returned: items.len(),
        items,
        groups,
        notes,
    }
}

pub fn deducted_profit_rule_notes(
    universe: &[StockItem],
    criteria: &ScreenCriteria,
) -> Vec<String> {
    if criteria.min_deducted_net_profit_billion.is_none()
        && criteria.min_deducted_net_profit_margin.is_none()
        && criteria.min_deducted_net_profit_growth_rate.is_none()
    {
        return Vec::new();
    }
    let with_metrics = universe
        .iter()
        .filter(|stock| has_deducted_profit_rule_metrics(stock, criteria))
        .count();
    if with_metrics < universe.len() {
        vec![format!("扣非净利润规则已启用；当前股票池 {with_metrics}/{} 只股票带扣非财务字段，缺字段股票按不达标处理。", universe.len())]
    } else {
        Vec::new()
    }
}

pub fn has_deducted_profit_rule_metrics(stock: &StockItem, criteria: &ScreenCriteria) -> bool {
    if criteria.min_deducted_net_profit_billion.is_some()
        && stock.deducted_net_profit_billion.is_none()
    {
        return false;
    }
    if criteria.min_deducted_net_profit_margin.is_some()
        && stock.deducted_net_profit_margin.is_none()
    {
        return false;
    }
    if criteria.min_deducted_net_profit_growth_rate.is_some()
        && stock.deducted_net_profit_growth_rate.is_none()
    {
        return false;
    }
    true
}

pub fn graph_screen_stocks(
    universe: &[StockItem],
    _provider_relations: &[StockRelation],
    request: &GraphScreenRequest,
) -> GraphScreenResult {
    let mut criteria = request.criteria.clone();
    criteria.limit = request.limit.clamp(1, 100);
    let screen_result = screen_stocks(universe, &criteria);
    let center_context = GraphCenterContext {
        mode: "custom_criteria".to_string(),
        label: "自定义筛选".to_string(),
        codes: Vec::new(),
    };

    let mut signals: Vec<GraphStockSignal> = screen_result
        .items
        .into_iter()
        .map(|screened| {
            let score = round6(screened.score);
            GraphStockSignal {
                stock: screened.stock,
                base_score: score,
                relation_score: 0.0,
                final_score: score,
                suggested_weight: 0.0,
                reasons: screened.reasons,
                related: Vec::new(),
                explanation: SelectionExplanation {
                    basis: vec![screened.score_explanation],
                    ..SelectionExplanation::default()
                },
            }
        })
        .collect();
    signals.sort_by(|left, right| right.final_score.total_cmp(&left.final_score));
    signals.truncate(request.limit.clamp(1, 100));
    assign_weights(&mut signals);

    let mut notes = vec![
        "自定义筛选使用页面筛选条件，不再依赖中心股票或关系传播。".to_string(),
        "旧关系图入口已降级为自定义筛选兼容结果，relation_count 固定为 0。".to_string(),
    ];
    notes.extend(screen_result.notes);

    GraphScreenResult {
        total: screen_result.total,
        returned: signals.len(),
        relation_count: 0,
        items: signals,
        center_context,
        notes,
    }
}
