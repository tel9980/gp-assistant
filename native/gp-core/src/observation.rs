//! 观察页数据构建：财务指标（含同比/环比口径）与资金证据（分区、评分、置信度）。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use observation::*;` 保证调用点不变。
use crate::*;
use serde_json::{json, Value};

pub fn build_observation_financial_indicators(
    stock: &StockItem,
    financial: Option<&StockFinancialSnapshot>,
) -> FinancialIndicators {
    let mut items = Vec::new();
    let mut notes = Vec::new();
    let mut source_parts = vec!["Tauri/Rust".to_string()];
    let mut period = "\u{672c}\u{5730}\u{884c}\u{60c5}\u{5feb}\u{7167}".to_string();

    if let Some(snapshot) = financial {
        if let Some(source) = snapshot
            .source
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            source_parts.push(source.clone());
        }
        if let Some(snapshot_period) = snapshot
            .period
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            period = snapshot_period.clone();
        }
    }
    let snapshot_period = financial.and_then(|snapshot| snapshot.period.as_deref());

    push_indicator(
        &mut items,
        "\u{5e02}\u{76c8}\u{7387}(TTM)",
        stock.pe,
        format_number,
        "neutral",
    );
    push_indicator(
        &mut items,
        "\u{5e02}\u{51c0}\u{7387}",
        stock.pb,
        format_number,
        "neutral",
    );
    let snapshot_roe = financial.and_then(|snapshot| snapshot.roe);
    if snapshot_roe.is_some() {
        push_indicator_with_meta(
            &mut items,
            "ROE",
            snapshot_roe,
            format_percentage_points,
            indicator_tone(snapshot_roe),
            Some("%"),
            Some("roe"),
            snapshot_period,
        );
    } else {
        push_indicator_with_meta(
            &mut items,
            "ROE",
            stock.roe,
            format_percent,
            indicator_tone(stock.roe),
            Some("%"),
            Some("roe"),
            None,
        );
    }
    push_indicator(
        &mut items,
        "\u{5e02}\u{503c}",
        stock.market_cap_billion,
        |value| format!("{}\u{4ebf}", format_number(value)),
        "neutral",
    );
    let snapshot_dividend_yield = financial.and_then(|snapshot| snapshot.dividend_yield);
    if snapshot_dividend_yield.is_some() {
        push_indicator_with_meta(
            &mut items,
            "\u{80a1}\u{606f}\u{7387}",
            snapshot_dividend_yield,
            format_percentage_points,
            "neutral",
            Some("%"),
            Some("dividend_yield"),
            financial.and_then(|snapshot| snapshot.dividend_period.as_deref()),
        );
    } else {
        push_indicator_with_meta(
            &mut items,
            "\u{80a1}\u{606f}\u{7387}",
            stock.dividend_yield,
            format_percent,
            "neutral",
            Some("%"),
            Some("dividend_yield"),
            None,
        );
    }
    push_indicator_with_meta(
        &mut items,
        "营业总收入",
        financial.and_then(|snapshot| snapshot.operating_revenue_billion),
        |value| format!("{}亿", format_number(value)),
        "neutral",
        Some("亿"),
        Some("operating_revenue"),
        snapshot_period,
    );
    push_indicator_with_meta(
        &mut items,
        "总营收同比",
        financial.and_then(|snapshot| snapshot.operating_revenue_yoy),
        format_percentage_points,
        indicator_tone(financial.and_then(|snapshot| snapshot.operating_revenue_yoy)),
        Some("%"),
        Some("operating_revenue_yoy"),
        snapshot_period,
    );
    push_indicator_with_meta(
        &mut items,
        "归母净利润",
        financial.and_then(|snapshot| snapshot.parent_net_profit_billion),
        |value| format!("{}亿", format_number(value)),
        indicator_tone(financial.and_then(|snapshot| snapshot.parent_net_profit_billion)),
        Some("亿"),
        Some("parent_net_profit"),
        snapshot_period,
    );
    push_indicator_with_meta(
        &mut items,
        "归母净利同比",
        financial.and_then(|snapshot| snapshot.parent_net_profit_yoy),
        format_percentage_points,
        indicator_tone(financial.and_then(|snapshot| snapshot.parent_net_profit_yoy)),
        Some("%"),
        Some("parent_net_profit_yoy"),
        snapshot_period,
    );
    for (label, metric_key, value, metric_period) in [
        (
            "毛利率",
            "gross_margin",
            financial.and_then(|snapshot| snapshot.gross_margin),
            snapshot_period,
        ),
        (
            "净利率",
            "net_margin",
            financial.and_then(|snapshot| snapshot.net_margin),
            snapshot_period,
        ),
        (
            "资产负债率",
            "asset_liability_ratio",
            financial.and_then(|snapshot| snapshot.asset_liability_ratio),
            snapshot_period,
        ),
        (
            "商誉净资产比",
            "goodwill_to_net_assets",
            financial.and_then(|snapshot| snapshot.goodwill_to_net_assets),
            financial.and_then(|snapshot| snapshot.goodwill_period.as_deref()),
        ),
        (
            "质押总股本比",
            "pledged_share_ratio",
            financial.and_then(|snapshot| snapshot.pledged_share_ratio),
            financial.and_then(|snapshot| snapshot.pledged_share_period.as_deref()),
        ),
        (
            "股利支付率(静)",
            "dividend_payout_ratio",
            financial.and_then(|snapshot| snapshot.dividend_payout_ratio),
            financial.and_then(|snapshot| snapshot.dividend_period.as_deref()),
        ),
    ] {
        push_indicator_with_meta(
            &mut items,
            label,
            value,
            format_percentage_points,
            "neutral",
            Some("%"),
            Some(metric_key),
            metric_period,
        );
    }
    push_indicator(
        &mut items,
        "\u{6263}\u{975e}\u{51c0}\u{5229}\u{6da6}",
        stock.deducted_net_profit_billion,
        |value| format!("{}\u{4ebf}", format_number(value)),
        indicator_tone(stock.deducted_net_profit_billion),
    );
    push_indicator(
        &mut items,
        "\u{6263}\u{975e}\u{51c0}\u{5229}\u{7387}",
        stock.deducted_net_profit_margin,
        format_percentage_points,
        "neutral",
    );
    push_indicator(
        &mut items,
        "\u{6263}\u{975e}\u{51c0}\u{5229}\u{6da6}\u{589e}\u{957f}\u{7387}",
        stock.deducted_net_profit_growth_rate,
        format_percentage_points,
        indicator_tone(stock.deducted_net_profit_growth_rate),
    );

    let latest_eps = financial.and_then(|snapshot| snapshot.latest_eps);
    if latest_eps.is_some() {
        push_indicator_with_meta(
            &mut items,
            "\u{6700}\u{65b0}\u{6bcf}\u{80a1}\u{6536}\u{76ca}",
            latest_eps,
            |value| format!("{}\u{5143}", format_number(value)),
            indicator_tone(latest_eps),
            Some("\u{5143}"),
            Some("latest_eps"),
            snapshot_period,
        );
    } else if let Some(pe) = stock.pe.filter(|value| value.abs() > f64::EPSILON) {
        let eps = stock.price / pe;
        push_indicator_with_meta(
            &mut items,
            "\u{6bcf}\u{80a1}\u{6536}\u{76ca}(\u{4f30}\u{7b97})",
            Some(eps),
            |value| format!("{}\u{5143}", format_number(value)),
            indicator_tone(Some(eps)),
            Some("\u{5143}"),
            Some("estimated_eps"),
            None,
        );
    }

    let latest_bps = financial.and_then(|snapshot| snapshot.latest_bps);
    if latest_bps.is_some() {
        push_indicator_with_meta(
            &mut items,
            "\u{6bcf}\u{80a1}\u{51c0}\u{8d44}\u{4ea7}",
            latest_bps,
            |value| format!("{}\u{5143}", format_number(value)),
            "neutral",
            Some("\u{5143}"),
            Some("latest_bps"),
            snapshot_period,
        );
    } else if let Some(pb) = stock.pb.filter(|value| value.abs() > f64::EPSILON) {
        let bps = stock.price / pb;
        push_indicator_with_meta(
            &mut items,
            "\u{6bcf}\u{80a1}\u{51c0}\u{8d44}\u{4ea7}(\u{4f30}\u{7b97})",
            Some(bps),
            |value| format!("{}\u{5143}", format_number(value)),
            "neutral",
            Some("\u{5143}"),
            Some("estimated_bps"),
            None,
        );
    }

    if let Some(snapshot) = financial {
        notes.extend(snapshot.notes.iter().filter_map(|note| {
            let trimmed = note.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }));
        let mut points = snapshot
            .quarterly_eps
            .iter()
            .filter(|point| {
                point.value.is_finite() && normalize_period_key(&point.period).is_some()
            })
            .cloned()
            .collect::<Vec<_>>();
        points.sort_by(|left, right| right.period.cmp(&left.period));
        points.dedup_by(|left, right| left.period == right.period);
        let values_by_period: HashMap<String, f64> = points
            .iter()
            .map(|point| (point.period.clone(), point.value))
            .collect();
        for point in points.iter().take(12) {
            let tone = previous_year_period(&point.period)
                .and_then(|previous| values_by_period.get(&previous).copied())
                .map(|previous| indicator_tone(Some(point.value - previous)))
                .unwrap_or("neutral");
            push_indicator_with_meta(
                &mut items,
                &format!("{} \u{6bcf}\u{80a1}\u{6536}\u{76ca}", point.period),
                Some(point.value),
                |value| format!("{}\u{5143}", format_number(value)),
                tone,
                Some("\u{5143}"),
                Some("quarterly_eps"),
                Some(point.period.as_str()),
            );
        }
        if points.is_empty() {
            notes
                .push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
        }
    } else {
        notes.push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
    }

    FinancialIndicators {
        title: "\u{6700}\u{65b0}\u{6307}\u{6807}".to_string(),
        period,
        source: source_parts.join(" / "),
        items,
        notes,
    }
}

pub fn previous_year_period(period: &str) -> Option<String> {
    let normalized = normalize_period_key(period)?;
    let year = normalized.get(0..4)?.parse::<i32>().ok()?;
    let quarter = normalized.get(5..6)?;
    Some(format!("{:04}Q{}", year - 1, quarter))
}

pub fn normalize_period_key(period: &str) -> Option<String> {
    let raw = period.trim().to_ascii_uppercase();
    let bytes = raw.as_bytes();
    if bytes.len() == 6
        && bytes[0..4].iter().all(|byte| byte.is_ascii_digit())
        && bytes[4] == b'Q'
        && matches!(bytes[5], b'1'..=b'4')
    {
        return Some(raw);
    }
    None
}
pub fn push_indicator(
    items: &mut Vec<FinancialIndicatorItem>,
    label: &str,
    raw_value: Option<f64>,
    formatter: impl Fn(f64) -> String,
    tone: &str,
) {
    push_indicator_with_meta(items, label, raw_value, formatter, tone, None, None, None);
}

#[allow(clippy::too_many_arguments)] // 参数即上下文，拆结构体反而更难读
pub fn push_indicator_with_meta(
    items: &mut Vec<FinancialIndicatorItem>,
    label: &str,
    raw_value: Option<f64>,
    formatter: impl Fn(f64) -> String,
    tone: &str,
    unit: Option<&str>,
    metric_key: Option<&str>,
    period: Option<&str>,
) {
    let Some(value) = raw_value.filter(|value| value.is_finite()) else {
        return;
    };
    items.push(FinancialIndicatorItem {
        label: label.to_string(),
        value: formatter(value),
        raw_value: Some(value),
        unit: unit.map(str::to_string),
        tone: tone.to_string(),
        metric_key: metric_key.map(str::to_string),
        period: period.map(str::to_string),
    });
}

pub fn format_number(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.0}")
    } else if value.abs() >= 10.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.3}")
    }
}

pub fn format_percent(value: f64) -> String {
    match ratio_percentage_points(value) {
        Some(percent) => format!("{}%", format_number(percent)),
        None => "—".to_string(),
    }
}

pub fn format_percentage_points(value: f64) -> String {
    match percentage_points(value) {
        Some(percent) => format!("{}%", format_number(percent)),
        None => "—".to_string(),
    }
}

pub fn indicator_tone(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value.is_finite() && value >= 0.0 => "rise",
        Some(value) if value.is_finite() => "fall",
        _ => "neutral",
    }
}

pub fn build_observation_capital_evidence(
    stock: &StockItem,
    _financial_indicators: &FinancialIndicators,
    trend: Option<&TrendIndicatorResult>,
    end_date: &str,
    provided: Option<CapitalEvidenceResult>,
) -> CapitalEvidenceResult {
    const FUND_FLOW_WEIGHT: f64 = 0.35;
    const INSTITUTION_WEIGHT: f64 = 0.25;
    const NEWS_WEIGHT: f64 = 0.15;
    const TECHNICAL_WEIGHT: f64 = 0.25;

    let as_of_trade_date = provided
        .as_ref()
        .and_then(|evidence| evidence.as_of_trade_date.as_deref())
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| capital_trade_date(value).or_else(|| Some(value.trim().to_string())))
        .or_else(|| capital_trade_date(end_date))
        .or_else(|| trend.map(|trend| trend.signal.date.clone()))
        .unwrap_or_else(|| Local::now().date_naive().format("%Y-%m-%d").to_string());

    let mut items = provided
        .as_ref()
        .map(|evidence| evidence.items.clone())
        .unwrap_or_default();
    items
        .retain(|item| item.category != "external_status" && item.category != "technical_behavior");

    let technical_score = trend.map(technical_capital_score);
    if let Some(trend) = trend {
        if !capital_bucket_has_evidence(&items, ["fund_flow", "", ""]) {
            if let Some(item) = local_fund_flow_proxy_evidence_item(trend) {
                items.push(item);
            }
        }
        items.push(technical_capital_evidence_item(
            trend,
            technical_score.unwrap_or(50.0),
        ));
    }
    if !capital_bucket_has_evidence(&items, ["institution_lhb", "institution_lhb_status", ""]) {
        items.push(institution_status_evidence_item(&as_of_trade_date));
    }
    if !capital_bucket_has_evidence(&items, ["news_rag", "community_sentiment", ""]) {
        items.push(message_sentiment_status_evidence_item(&as_of_trade_date));
    }
    dedup_capital_items(&mut items);

    let buckets = [
        (
            "资金流",
            FUND_FLOW_WEIGHT,
            ["fund_flow", "", ""],
            ["fund_flow", "", ""],
        ),
        (
            "机构席位",
            INSTITUTION_WEIGHT,
            ["institution_lhb", "institution_lhb_status", ""],
            ["institution_lhb", "", ""],
        ),
        (
            "消息情绪",
            NEWS_WEIGHT,
            [
                "news_rag",
                "community_sentiment",
                "message_sentiment_status",
            ],
            ["news_rag", "community_sentiment", ""],
        ),
        (
            "技术推断",
            TECHNICAL_WEIGHT,
            ["technical_behavior", "", ""],
            ["technical_behavior", "", ""],
        ),
    ];
    let mut weighted_sum = 0.0;
    let mut total_weight: f64 = 0.0;
    let mut contributions = BTreeMap::new();
    let mut scored_labels = Vec::new();
    let mut evidence_labels = Vec::new();
    for (label, weight, score_categories, evidence_categories) in buckets {
        let score = capital_bucket_score(&items, score_categories);
        let available = capital_bucket_has_evidence(&items, evidence_categories);
        if score.is_some() {
            scored_labels.push(label.to_string());
        }
        if available {
            evidence_labels.push(label.to_string());
        }
        let bucket_score = score.unwrap_or(50.0);
        weighted_sum += bucket_score * weight;
        total_weight += weight;
        contributions.insert(
            label.to_string(),
            json!({
                "score": score.map(round2),
                "weight": weight,
                "available": available,
            }),
        );
    }
    let composite_score = round2(weighted_sum / total_weight.max(0.01));
    let model_used = provided
        .as_ref()
        .map(|evidence| evidence.model_used)
        .unwrap_or(false);
    let confidence = capital_evidence_confidence(&items, technical_score.is_some());
    let sections = build_capital_evidence_sections(&items, &contributions);
    let freshness = provided
        .as_ref()
        .map(|evidence| evidence.freshness.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("refreshed")
        .to_string();
    let mut notes = Vec::new();
    if let Some(provided) = provided.as_ref() {
        for note in &provided.notes {
            push_capital_note(&mut notes, note);
        }
    }
    if !model_used {
        push_capital_note(
            &mut notes,
            "未调用模型，综合资金证据分由本地规则按资金流、机构席位、消息情绪和技术推断四类权重生成。",
        );
    }
    if !capital_bucket_has_evidence(&items, ["fund_flow", "", ""]) {
        push_capital_note(
            &mut notes,
            "资金流外部证据暂缺，本次按中性权重保留该项，不把缺失数据当作流入或流出结论。",
        );
    }
    if !items.iter().any(|item| item.category == "institution_lhb") {
        push_capital_note(
            &mut notes,
            "龙虎榜机构席位未命中或未提供，仅表示当前证据源没有可展示记录，不代表机构没有交易。",
        );
    }
    if !capital_bucket_has_evidence(&items, ["news_rag", "community_sentiment", ""]) {
        push_capital_note(
            &mut notes,
            "消息情绪证据暂缺，当前不对新闻或社区情绪加减分。",
        );
    }

    CapitalEvidenceResult {
        stock_code: normalize_stock_code(&stock.code),
        generated_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        composite_score: Some(composite_score),
        confidence,
        model_used,
        as_of_trade_date: Some(as_of_trade_date),
        freshness,
        contributions,
        summary: Some(capital_evidence_summary(
            composite_score,
            &scored_labels,
            &evidence_labels,
        )),
        sections,
        items,
        notes,
    }
}

pub fn dedup_capital_items(items: &mut Vec<CapitalEvidenceItem>) {
    let mut seen = HashSet::new();
    items.retain(|item| {
        let key = format!(
            "{}|{}|{}|{}",
            item.category,
            item.source,
            item.title,
            item.date.as_deref().unwrap_or("")
        );
        seen.insert(key)
    });
}

pub fn capital_item_matches<const N: usize>(
    item: &CapitalEvidenceItem,
    categories: [&str; N],
) -> bool {
    categories
        .iter()
        .any(|category| !category.is_empty() && *category == item.category.as_str())
}

pub fn capital_bucket_has_evidence<const N: usize>(
    items: &[CapitalEvidenceItem],
    categories: [&str; N],
) -> bool {
    items
        .iter()
        .any(|item| capital_item_matches(item, categories))
}

pub fn capital_bucket_score<const N: usize>(
    items: &[CapitalEvidenceItem],
    categories: [&str; N],
) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0.0;
    for item in items
        .iter()
        .filter(|item| capital_item_matches(item, categories))
    {
        if let Some(score) = item.score.filter(|value| value.is_finite()) {
            total += score;
            count += 1.0;
        }
    }
    if count > 0.0 {
        Some(round2(total / count))
    } else {
        None
    }
}

pub fn capital_evidence_confidence(items: &[CapitalEvidenceItem], has_technical: bool) -> String {
    let scored_external_count = [
        ["fund_flow", "", ""],
        ["institution_lhb", "", ""],
        [
            "news_rag",
            "community_sentiment",
            "message_sentiment_status",
        ],
    ]
    .into_iter()
    .filter(|categories| capital_bucket_score(items, *categories).is_some())
    .count();
    if scored_external_count >= 3 {
        "高".to_string()
    } else if scored_external_count >= 1 || has_technical {
        "中".to_string()
    } else {
        "低".to_string()
    }
}

pub fn capital_evidence_summary(
    composite_score: f64,
    scored_labels: &[String],
    evidence_labels: &[String],
) -> String {
    if !scored_labels.is_empty() {
        return format!(
            "综合资金证据分 {}，已纳入{}；缺失分数的证据项按中性权重处理。",
            format_number(composite_score),
            scored_labels.join("、")
        );
    }
    if !evidence_labels.is_empty() {
        return format!(
            "综合资金证据分 {}，当前只有{}状态证据；缺失分数的证据项按中性权重处理。",
            format_number(composite_score),
            evidence_labels.join("、")
        );
    }
    format!(
        "综合资金证据分 {}，外部资金证据暂缺，当前仅保留中性权重。",
        format_number(composite_score)
    )
}

pub fn push_capital_note(notes: &mut Vec<String>, note: impl AsRef<str>) {
    let clean = note.as_ref().trim();
    if clean.is_empty() || notes.iter().any(|existing| existing == clean) {
        return;
    }
    notes.push(clean.to_string());
}

pub fn local_fund_flow_proxy_evidence_item(
    trend: &TrendIndicatorResult,
) -> Option<CapitalEvidenceItem> {
    let signal = &trend.signal;
    let latest = trend.series.last()?;
    let score = local_fund_flow_proxy_score(latest);
    let mut metrics = BTreeMap::new();
    metrics.insert("隐性资金代理分".to_string(), format_number(score));
    metrics.insert(
        "推断方向".to_string(),
        fund_flow_proxy_direction(score).to_string(),
    );
    metrics.insert("收盘价".to_string(), format_number(signal.close));
    if let Some(change_pct) = signal.close_change_pct {
        metrics.insert("涨跌幅".to_string(), format_percent(change_pct));
    }
    let numeric_metrics = [
        ("量价热度", latest.volume_price_heat),
        ("吸筹强度", latest.accumulation_strength),
        ("吸筹指标", latest.accumulation_index),
        ("趋势热度", latest.trend_heat),
        ("异动热度", latest.anomaly_heat),
        ("人气热度", latest.popularity_heat),
    ];
    for (label, value) in numeric_metrics {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            metrics.insert(label.to_string(), format_number(value));
        }
    }
    metrics.insert("证据类型".to_string(), "本地日线量价代理".to_string());
    Some(CapitalEvidenceItem {
        category: "fund_flow".to_string(),
        source: "Tauri/Rust 日线量价".to_string(),
        title: "本地量价资金代理".to_string(),
        date: Some(signal.date.clone()),
        metrics,
        sentiment: score_sentiment(score).to_string(),
        weight: 0.35,
        confidence: "中".to_string(),
        url: None,
        score: Some(score),
        note: Some(format!(
            "资金流代理分 {}：由量价热度、吸筹强度、趋势热度和承接指标合成；它不是外部主力净流入数据。",
            format_number(score)
        )),
    })
}

pub fn local_fund_flow_proxy_score(point: &TrendIndicatorPoint) -> f64 {
    let volume_price = point.volume_price_heat.unwrap_or(50.0).clamp(0.0, 100.0);
    let accumulation_strength = point
        .accumulation_strength
        .unwrap_or(50.0)
        .clamp(0.0, 100.0);
    let trend_heat = point.trend_heat.unwrap_or(50.0).clamp(0.0, 100.0);
    let rebound = point.rebound_signal.unwrap_or(50.0).clamp(0.0, 100.0);
    let accumulation_index = point
        .accumulation_index
        .map(|value| (50.0 + value * 2.0).clamp(0.0, 100.0))
        .unwrap_or(50.0);
    round2(
        volume_price * 0.35
            + accumulation_strength * 0.25
            + trend_heat * 0.20
            + accumulation_index * 0.15
            + rebound * 0.05,
    )
}

pub fn fund_flow_proxy_direction(score: f64) -> &'static str {
    if score >= 60.0 {
        "偏流入"
    } else if score <= 40.0 {
        "偏流出"
    } else {
        "中性"
    }
}

pub fn technical_capital_score(trend: &TrendIndicatorResult) -> f64 {
    let signal = &trend.signal;
    let quant_max = signal.quant_score_max.max(1) as f64;
    let pattern_max = signal.pattern_score_max.max(1) as f64;
    let quant = (signal.quant_score as f64 / quant_max * 100.0).clamp(0.0, 100.0);
    let pattern = (signal.pattern_score as f64 / pattern_max * 100.0).clamp(0.0, 100.0);
    let mut score = quant * 0.55 + pattern * 0.45;
    if signal.short_buy {
        score += 8.0;
    }
    if signal.kdj_golden_cross {
        score += 6.0;
    }
    if signal.kdj_oversold {
        score += 5.0;
    }
    if signal.red_hold {
        score += 4.0;
    }
    if signal.swl_above_sws {
        score += 3.0;
    }
    if signal.white_exit {
        score -= 12.0;
    }
    if signal.kdj_dead_cross {
        score -= 10.0;
    }
    if signal.kdj_overbought {
        score -= 8.0;
    }
    if signal.cyan_watch {
        score -= 4.0;
    }
    round2(score.clamp(0.0, 100.0))
}

pub fn technical_capital_evidence_item(
    trend: &TrendIndicatorResult,
    score: f64,
) -> CapitalEvidenceItem {
    let signal = &trend.signal;
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "\u{6536}\u{76d8}\u{4ef7}".to_string(),
        format_number(signal.close),
    );
    if let Some(change_pct) = signal.close_change_pct {
        metrics.insert(
            "\u{6da8}\u{8dcc}\u{5e45}".to_string(),
            format_percent(change_pct),
        );
    }
    metrics.insert(
        "\u{91cf}\u{5316}\u{5206}".to_string(),
        format!("{}/{}", signal.quant_score, signal.quant_score_max),
    );
    metrics.insert(
        "\u{5f62}\u{6001}\u{5206}".to_string(),
        format!("{}/{}", signal.pattern_score, signal.pattern_score_max),
    );
    if let (Some(k), Some(d), Some(j)) = (signal.k, signal.d, signal.j) {
        metrics.insert(
            "KDJ".to_string(),
            format!(
                "K {} / D {} / J {}",
                format_number(k),
                format_number(d),
                format_number(j)
            ),
        );
    }
    if let (Some(swl), Some(sws)) = (signal.swl, signal.sws) {
        metrics.insert(
            "SWL/SWS".to_string(),
            format!("{} / {}", format_number(swl), format_number(sws)),
        );
    }
    metrics.insert("\u{72b6}\u{6001}".to_string(), signal.status.clone());
    if let Some(point) = trend.series.last() {
        let point_metrics = [
            ("吸筹指标", point.accumulation_index),
            ("吸筹强度", point.accumulation_strength),
            ("波段机会", point.swing_opportunity),
            ("绝地反击", point.rebound_signal),
            ("趋势热度", point.trend_heat),
            ("量价热度", point.volume_price_heat),
            ("异动热度", point.anomaly_heat),
            ("人气热度", point.popularity_heat),
        ];
        for (label, value) in point_metrics {
            if let Some(value) = value.filter(|value| value.is_finite()) {
                metrics.insert(label.to_string(), format_number(value));
            }
        }
    }
    metrics.insert(
        "\u{77ed}\u{7ebf}\u{4e70}\u{70b9}".to_string(),
        if signal.short_buy {
            "\u{662f}"
        } else {
            "\u{5426}"
        }
        .to_string(),
    );
    let risk_labels = capital_technical_risk_labels(signal);
    if !risk_labels.is_empty() {
        metrics.insert(
            "\u{98ce}\u{9669}\u{4fe1}\u{53f7}".to_string(),
            risk_labels.join("\u{3001}"),
        );
    }
    CapitalEvidenceItem {
        category: "technical_behavior".to_string(),
        source: "Tauri/Rust \u{6280}\u{672f}\u{6307}\u{6807}".to_string(),
        title: "\u{6280}\u{672f}\u{63a8}\u{65ad}\u{8d44}\u{91d1}\u{627f}\u{63a5}".to_string(),
        date: Some(signal.date.clone()),
        metrics,
        sentiment: score_sentiment(score).to_string(),
        weight: 0.25,
        confidence: "\u{4e2d}".to_string(),
        url: None,
        score: Some(score),
        note: Some(technical_capital_note(signal, score)),
    }
}

pub fn institution_status_evidence_item(as_of_trade_date: &str) -> CapitalEvidenceItem {
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "状态".to_string(),
        "当前没有可展示的龙虎榜机构席位记录".to_string(),
    );
    metrics.insert("查询窗口".to_string(), as_of_trade_date.to_string());
    metrics.insert(
        "已尝试信源".to_string(),
        "本地缓存 / 已提供外部证据".to_string(),
    );
    CapitalEvidenceItem {
        category: "institution_lhb_status".to_string(),
        source: "Tauri/Rust".to_string(),
        title: "机构席位暂无外部证据".to_string(),
        date: Some(as_of_trade_date.to_string()),
        metrics,
        sentiment: "uncertain".to_string(),
        weight: 0.25,
        confidence: "低".to_string(),
        url: None,
        score: None,
        note: Some(
            "当前窗口没有可展示的龙虎榜机构席位证据；这只是证据缺口，不代表机构没有买卖。"
                .to_string(),
        ),
    }
}

pub fn message_sentiment_status_evidence_item(as_of_trade_date: &str) -> CapitalEvidenceItem {
    let mut metrics = BTreeMap::new();
    metrics.insert("状态".to_string(), "本地消息缓存暂无可用证据".to_string());
    metrics.insert("查询窗口".to_string(), as_of_trade_date.to_string());
    metrics.insert(
        "已尝试信源".to_string(),
        "本地新闻缓存 / 已提供外部证据".to_string(),
    );
    CapitalEvidenceItem {
        category: "message_sentiment_status".to_string(),
        source: "Tauri/Rust".to_string(),
        title: "消息情绪暂无外部证据".to_string(),
        date: Some(as_of_trade_date.to_string()),
        metrics,
        sentiment: "uncertain".to_string(),
        weight: 0.15,
        confidence: "低".to_string(),
        url: None,
        score: None,
        note: Some("当前窗口没有可展示的新闻或社区情绪证据；该项仅作为证据缺口提示。".to_string()),
    }
}
pub fn build_capital_evidence_sections(
    items: &[CapitalEvidenceItem],
    contributions: &BTreeMap<String, Value>,
) -> Vec<CapitalEvidenceSection> {
    let definitions = [
        (
            "fund_flow",
            "资金流",
            "资金流",
            ["fund_flow", "", ""],
            ["fund_flow", "", ""],
        ),
        (
            "institution_lhb",
            "机构席位",
            "机构席位",
            ["institution_lhb", "institution_lhb_status", ""],
            ["institution_lhb", "", ""],
        ),
        (
            "message_sentiment",
            "消息情绪",
            "消息情绪",
            [
                "news_rag",
                "community_sentiment",
                "message_sentiment_status",
            ],
            ["news_rag", "community_sentiment", ""],
        ),
        (
            "technical_behavior",
            "技术推断",
            "技术推断",
            ["technical_behavior", "", ""],
            ["technical_behavior", "", ""],
        ),
    ];
    definitions
        .into_iter()
        .map(
            |(key, title, contribution_key, categories, evidence_categories)| {
                let section_items = items
                    .iter()
                    .filter(|item| {
                        categories
                            .iter()
                            .any(|category| !category.is_empty() && *category == item.category)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let contribution = contributions.get(contribution_key);
                let score = contribution
                    .and_then(|value| value.get("score"))
                    .and_then(Value::as_f64);
                let weight = contribution
                    .and_then(|value| value.get("weight"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let contribution_available = contribution
                    .and_then(|value| value.get("available"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let has_real_evidence = section_items.iter().any(|item| {
                    evidence_categories
                        .iter()
                        .any(|category| !category.is_empty() && *category == item.category)
                });
                let available = contribution_available || has_real_evidence;
                CapitalEvidenceSection {
                    key: key.to_string(),
                    title: title.to_string(),
                    score,
                    weight,
                    available,
                    summary: Some(capital_section_summary(
                        title,
                        score,
                        available,
                        section_items.len(),
                    )),
                    items: section_items,
                }
            },
        )
        .collect()
}

pub fn capital_section_summary(
    title: &str,
    score: Option<f64>,
    available: bool,
    item_count: usize,
) -> String {
    if available {
        if let Some(score) = score {
            return format!("{}\u{8bc1}\u{636e}\u{5206} {}\u{ff0c}\u{547d}\u{4e2d} {} \u{6761}\u{8bc1}\u{636e}\u{3002}", title, format_number(score), item_count);
        }
        return format!("{}\u{6709} {} \u{6761}\u{72b6}\u{6001}\u{6216}\u{8f85}\u{52a9}\u{8bc1}\u{636e}\u{3002}", title, item_count);
    }
    format!(
        "{}\u{6682}\u{65e0}\u{53ef}\u{7528}\u{8bc1}\u{636e}\u{3002}",
        title
    )
}

pub fn capital_technical_risk_labels(signal: &TrendIndicatorSignal) -> Vec<String> {
    let mut labels = Vec::new();
    if signal.white_exit {
        labels.push("\u{767d}\u{7ebf}\u{5356}\u{51fa}".to_string());
    }
    if signal.kdj_dead_cross {
        labels.push("KDJ \u{6b7b}\u{53c9}".to_string());
    }
    if signal.kdj_overbought {
        labels.push("KDJ \u{9ad8}\u{4f4d}\u{8d85}\u{4e70}".to_string());
    }
    if signal.cyan_watch {
        labels.push("\u{9752}\u{8272}\u{89c2}\u{671b}".to_string());
    }
    labels
}

pub fn technical_capital_note(signal: &TrendIndicatorSignal, score: f64) -> String {
    let mut parts = Vec::new();
    if signal.short_buy {
        parts.push("\u{77ed}\u{7ebf}\u{4e70}\u{70b9}\u{6210}\u{7acb}".to_string());
    }
    if signal.kdj_golden_cross {
        parts.push("KDJ \u{91d1}\u{53c9}".to_string());
    }
    if signal.swl_above_sws {
        parts.push("SWL \u{5728} SWS \u{4e0a}\u{65b9}".to_string());
    }
    if signal.white_exit || signal.kdj_dead_cross || signal.kdj_overbought {
        parts.push("\u{5b58}\u{5728}\u{6280}\u{672f}\u{98ce}\u{9669}\u{4fe1}\u{53f7}".to_string());
    }
    if parts.is_empty() {
        parts.push("\u{6280}\u{672f}\u{6307}\u{6807}\u{672a}\u{51fa}\u{73b0}\u{5f3a}\u{52bf}\u{627f}\u{63a5}".to_string());
    }
    format!(
        "\u{6280}\u{672f}\u{63a8}\u{65ad}\u{5206} {}\u{ff1a}{}\u{3002}\u{8be5}\u{9879}\u{53ea}\u{4f5c}\u{8d44}\u{91d1}\u{884c}\u{4e3a}\u{7684}\u{8f85}\u{52a9}\u{89c2}\u{5bdf}\u{ff0c}\u{4e0d}\u{7b49}\u{540c}\u{4e8e}\u{771f}\u{5b9e}\u{8d44}\u{91d1}\u{6d41}\u{3002}",
        format_number(score),
        parts.join("\u{3001}")
    )
}
