//! 板块与分组：概念/板块归类与排名、主题匹配、冷门行业判断、热门提升、行业相对分与分散度、结果分组。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use sector::*;` 保证调用点不变。
use crate::*;

pub fn concept_group_for_stock(stock: &StockItem) -> String {
    let text = stock_text(stock);
    for (label, keywords) in CONCEPT_GROUP_RULES {
        if contains_any(&text, keywords) {
            return label.to_string();
        }
    }
    "\u{5176}\u{4ed6}\u{6982}\u{5ff5}".to_string()
}

pub fn concept_sector_group_for_stock(stock: &StockItem) -> String {
    let concept = concept_group_for_stock(stock);
    let semiconductor_group = matches!(
        concept.as_str(),
        "半导体设计"
            | "半导体设备"
            | "半导体材料"
            | "半导体晶圆"
            | "半导体封测"
            | "存储芯片"
            | "半导体"
    ) || (concept == "AI算力与芯片"
        && contains_any(
            &stock_text(stock),
            &["半导体", "集成电路", "芯片制造", "芯片产业"],
        ));
    if semiconductor_group {
        "半导体".to_string()
    } else {
        concept
    }
}

pub fn concept_rank(concept: &str) -> usize {
    CONCEPT_GROUP_RULES
        .iter()
        .position(|(label, _)| *label == concept)
        .unwrap_or(CONCEPT_GROUP_RULES.len())
}

pub fn board_group_for_stock(stock: &StockItem) -> &'static str {
    let code = stock.code.trim().to_ascii_uppercase();
    let digits = code.split('.').next().unwrap_or("");
    if code.ends_with(".BJ")
        || digits.starts_with('8')
        || digits.starts_with('4')
        || digits.starts_with('9')
    {
        "\u{5317}\u{4ea4}\u{6240}"
    } else if digits.starts_with("688") {
        "\u{79d1}\u{521b}\u{677f}"
    } else if digits.starts_with("300") || digits.starts_with("301") {
        "\u{521b}\u{4e1a}\u{677f}"
    } else if code.ends_with(".SH") || digits.starts_with('6') {
        "\u{6caa}\u{4e3b}\u{677f}"
    } else {
        "\u{6df1}\u{4e3b}\u{677f}"
    }
}

pub fn board_rank(board: &str) -> usize {
    match board {
        "\u{79d1}\u{521b}\u{677f}" => 0,
        "\u{521b}\u{4e1a}\u{677f}" => 1,
        "\u{5317}\u{4ea4}\u{6240}" => 2,
        "\u{6caa}\u{4e3b}\u{677f}" => 3,
        "\u{6df1}\u{4e3b}\u{677f}" => 4,
        _ => 99,
    }
}

pub fn theme_match_for_stock(stock: &StockItem) -> Option<(&'static str, &'static str, f64)> {
    theme_match_for_text(&stock_text(stock))
}

pub fn theme_match_for_text(text: &str) -> Option<(&'static str, &'static str, f64)> {
    for (key, label, score, keywords) in THEME_RULES {
        if contains_any(text, keywords) {
            return Some((key, label, score));
        }
    }
    None
}

pub fn stock_text(stock: &StockItem) -> String {
    format!("{} {}", stock.name, stock.industry).to_lowercase()
}

pub fn is_cold_sector(industry: &str) -> bool {
    let normalized = industry.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    COLD_SECTOR_KEYWORDS
        .iter()
        .any(|keyword| normalized.contains(keyword))
}

pub fn should_promote_hot_sectors(criteria: &ScreenCriteria) -> bool {
    criteria.industry.as_deref().unwrap_or("").trim().is_empty()
        && criteria.sort_by.trim().eq_ignore_ascii_case("score")
        && !criteria.sort_dir.trim().eq_ignore_ascii_case("asc")
        && normalized_score_profile(&criteria.score_profile) != "quality"
}

pub fn promote_hot_sector_items(
    screened: &[ScreenedStock],
    criteria: &ScreenCriteria,
    limit: usize,
) -> Vec<ScreenedStock> {
    if !should_promote_hot_sectors(criteria) || limit == 0 {
        return screened.iter().take(limit).cloned().collect();
    }

    let mut promoted = Vec::new();
    let mut used_codes = HashSet::new();
    for category in THEME_PROMOTION_ORDER {
        if let Some(candidate) = screened.iter().find(|item| {
            !used_codes.contains(&item.stock.code)
                && item.theme_category.as_deref() == Some(category)
        }) {
            promoted.push(candidate.clone());
            used_codes.insert(candidate.stock.code.clone());
            if promoted.len() >= limit {
                return promoted;
            }
        }
    }

    for category in THEME_FILL_ORDER {
        for item in screened {
            if used_codes.contains(&item.stock.code)
                || item.theme_category.as_deref() != Some(category)
            {
                continue;
            }
            promoted.push(item.clone());
            used_codes.insert(item.stock.code.clone());
            if promoted.len() >= limit {
                return promoted;
            }
        }
    }

    for item in screened {
        if used_codes.contains(&item.stock.code) || is_cold_sector(&item.stock.industry) {
            continue;
        }
        promoted.push(item.clone());
        used_codes.insert(item.stock.code.clone());
        if promoted.len() >= limit {
            return promoted;
        }
    }

    for item in screened {
        if used_codes.contains(&item.stock.code) {
            continue;
        }
        promoted.push(item.clone());
        used_codes.insert(item.stock.code.clone());
        if promoted.len() >= limit {
            break;
        }
    }
    promoted
}

pub fn apply_industry_relative_scores(screened: &mut [ScreenedStock], score_profile: &str) {
    let quality_values = industry_percentiles(screened, "fundamental");
    let valuation_values = industry_percentiles(screened, "valuation");
    let profile = normalized_score_profile(score_profile);

    for item in screened.iter_mut() {
        let quality_pct = quality_values.get(&item.stock.code).copied().unwrap_or(0.5);
        let valuation_pct = valuation_values
            .get(&item.stock.code)
            .copied()
            .unwrap_or(0.5);
        let relative_score = ((quality_pct + valuation_pct) / 2.0).clamp(0.0, 1.0);
        let old_quality = item.quality_score;
        let old_balanced = item.balanced_score;
        item.quality_score = round6(
            (old_quality * 0.82 + relative_score * SCREEN_SCORE_SCALE * 0.18)
                .clamp(0.0, SCREEN_SCORE_SCALE),
        );
        item.balanced_score = round6(
            (old_balanced * 0.88 + relative_score * SCREEN_SCORE_SCALE * 0.12)
                .clamp(0.0, SCREEN_SCORE_SCALE),
        );
        item.factor_scores
            .insert("industry_quality_pct".to_string(), round6(quality_pct));
        item.factor_scores
            .insert("industry_valuation_pct".to_string(), round6(valuation_pct));
        item.factor_scores.insert(
            "industry_relative_score".to_string(),
            round6(relative_score),
        );
        item.reason_tags.push("行业内相对评分".to_string());

        item.score = match profile {
            "quality" => item.quality_score,
            "balanced" => item.balanced_score,
            _ => item.score,
        };
        if matches!(profile, "quality" | "balanced") {
            let retained = if profile == "quality" { 0.82 } else { 0.88 };
            for entry in &mut item.score_breakdown {
                entry.contribution = entry.contribution.map(|value| round6(value * retained));
            }
            item.score_breakdown.push(ScoreContribution {
                key: "industry_relative_score".to_string(),
                label: "行业内相对评分".to_string(),
                value: Some(round6(relative_score * SCREEN_SCORE_SCALE)),
                contribution: Some(round6(
                    relative_score * SCREEN_SCORE_SCALE * (1.0 - retained),
                )),
                tone: "neutral".to_string(),
            });
            reconcile_score_breakdown(&mut item.score_breakdown, item.score);
            item.score_explanation = explain_score(&item.score_breakdown);
        }
    }
}

pub fn industry_percentiles(screened: &[ScreenedStock], factor_key: &str) -> HashMap<String, f64> {
    industry_rank_values(screened.iter().map(|item| {
        (
            item.stock.code.clone(),
            item.stock.industry.clone(),
            item.factor_scores.get(factor_key).copied().unwrap_or(0.5),
        )
    }))
    .into_iter()
    .map(|(code, (rank, _))| (code, rank))
    .collect()
}

pub fn diversify_by_industry(screened: &[ScreenedStock], limit: usize) -> Vec<ScreenedStock> {
    if limit == 0 || screened.len() <= 3 {
        return screened.iter().take(limit).cloned().collect();
    }
    let per_industry_limit = ((limit as f64) * 0.45).ceil().max(2.0) as usize;
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut selected = Vec::new();
    let mut deferred = Vec::new();

    for item in screened {
        let industry = if item.stock.industry.trim().is_empty() {
            "__unknown__".to_string()
        } else {
            item.stock.industry.trim().to_string()
        };
        let count = counts.get(&industry).copied().unwrap_or(0);
        if count < per_industry_limit {
            counts.insert(industry, count + 1);
            selected.push(item.clone());
        } else {
            deferred.push(item.clone());
        }
        if selected.len() >= limit {
            return selected;
        }
    }

    for item in deferred {
        selected.push(item);
        if selected.len() >= limit {
            break;
        }
    }
    selected
}
pub fn sort_screened(screened: &mut [ScreenedStock], criteria: &ScreenCriteria) {
    let sort_by = criteria.sort_by.trim().to_ascii_lowercase();
    let ascending = criteria.sort_dir.trim().eq_ignore_ascii_case("asc");
    screened.sort_by(|left, right| {
        let ordering = match (sort_value(left, &sort_by), sort_value(right, &sort_by)) {
            (Some(left_value), Some(right_value)) => {
                if ascending {
                    left_value.total_cmp(&right_value)
                } else {
                    right_value.total_cmp(&left_value)
                }
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        };
        ordering
            .then_with(|| right.score.total_cmp(&left.score))
            .then_with(|| left.stock.code.cmp(&right.stock.code))
    });
}

pub fn should_diversify_by_industry(criteria: &ScreenCriteria) -> bool {
    criteria.industry.as_deref().unwrap_or("").trim().is_empty()
        && criteria.sort_by.trim().eq_ignore_ascii_case("score")
        && !criteria.sort_dir.trim().eq_ignore_ascii_case("asc")
        && normalized_score_profile(&criteria.score_profile) != "quality"
}

pub fn primary_screen_items(
    screened: &[ScreenedStock],
    criteria: &ScreenCriteria,
    limit: usize,
) -> (Vec<ScreenedStock>, bool) {
    if !should_promote_hot_sectors(criteria) || limit == 0 {
        let items = if should_diversify_by_industry(criteria) {
            diversify_by_industry(screened, limit)
        } else {
            screened.iter().take(limit).cloned().collect()
        };
        let changed = items
            .iter()
            .map(|item| item.stock.code.as_str())
            .collect::<Vec<_>>()
            != screened
                .iter()
                .take(limit)
                .map(|item| item.stock.code.as_str())
                .collect::<Vec<_>>();
        return (items, changed);
    }
    let promoted = promote_hot_sector_items(screened, criteria, limit);
    let promoted_flag = promoted
        .iter()
        .map(|item| item.stock.code.as_str())
        .collect::<Vec<_>>()
        != screened
            .iter()
            .take(limit)
            .map(|item| item.stock.code.as_str())
            .collect::<Vec<_>>();
    (promoted, promoted_flag)
}

pub fn screen_result_groups(screened: &[ScreenedStock]) -> Vec<ScreenResultGroup> {
    let hot_items = hot_group_items(screened, SCREEN_GROUP_LIMIT);
    let hot_codes: HashSet<String> = hot_items
        .iter()
        .map(|item| item.stock.code.clone())
        .collect();
    let potential_items = potential_group_items(
        screened,
        SCREEN_GROUP_LIMIT,
        &hot_codes,
        POTENTIAL_SCORE_THRESHOLD,
    );
    vec![
        ScreenResultGroup {
            key: "hot".to_string(),
            title: "\u{70ed}\u{95e8}\u{80a1}".to_string(),
            description: "AI\u{4e0a}\u{4e0b}\u{6e38}\u{3001}\u{82af}\u{7247}\u{3001}\u{7b97}\u{529b}\u{3001}\u{80fd}\u{6e90}\u{3001}\u{65b0}\u{6750}\u{6599}\u{3001}\u{533b}\u{836f}\u{3001}\u{6e38}\u{620f}\u{7b49}\u{70ed}\u{95e8}\u{65b9}\u{5411}\u{ff0c}\u{6309}\u{7efc}\u{5408}\u{5206}\u{548c}\u{4e3b}\u{9898}\u{4f18}\u{5148}\u{7ea7}\u{5c55}\u{793a}\u{3002}".to_string(),
            total: screened.iter().filter(|item| item.theme_category.is_some()).count(),
            returned: hot_items.len(),
            items: hot_items,
        },
        ScreenResultGroup {
            key: "potential".to_string(),
            title: "\u{6f5c}\u{529b}\u{80a1}".to_string(),
            description: format!("{}{}{}", "\u{7efc}\u{5408}\u{5206}\u{5927}\u{4e8e} ", POTENTIAL_SCORE_THRESHOLD, " \u{7684}\u{5019}\u{9009}\u{ff0c}\u{5df2}\u{5c3d}\u{91cf}\u{907f}\u{514d}\u{4e0e}\u{70ed}\u{95e8}\u{80a1}\u{91cd}\u{590d}\u{3002}"),
            total: screened.iter().filter(|item| item.score > POTENTIAL_SCORE_THRESHOLD).count(),
            returned: potential_items.len(),
            items: potential_items,
        },
    ]
}

pub fn hot_group_items(screened: &[ScreenedStock], limit: usize) -> Vec<ScreenedStock> {
    let mut selected = Vec::new();
    let mut used_codes = HashSet::new();
    let mut by_score = screened.to_vec();
    by_score.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.stock.code.cmp(&right.stock.code))
    });
    for category in THEME_PROMOTION_ORDER {
        if let Some(candidate) = first_by_theme(&by_score, category, &used_codes) {
            append_selected(&mut selected, &mut used_codes, candidate);
            if selected.len() >= limit {
                return selected;
            }
        }
    }
    for category in THEME_FILL_ORDER {
        for item in &by_score {
            if used_codes.contains(&item.stock.code)
                || item.theme_category.as_deref() != Some(category)
            {
                continue;
            }
            append_selected(&mut selected, &mut used_codes, item.clone());
            if selected.len() >= limit {
                return selected;
            }
        }
    }
    selected
}

pub fn potential_group_items(
    screened: &[ScreenedStock],
    limit: usize,
    exclude_codes: &HashSet<String>,
    threshold: f64,
) -> Vec<ScreenedStock> {
    let mut ranked = screened.to_vec();
    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.stock.code.cmp(&right.stock.code))
    });
    let mut selected = Vec::new();
    for item in ranked {
        if exclude_codes.contains(&item.stock.code) || item.score <= threshold {
            continue;
        }
        selected.push(item);
        if selected.len() >= limit {
            break;
        }
    }
    selected
}

pub fn first_by_theme(
    screened: &[ScreenedStock],
    category: &str,
    used_codes: &HashSet<String>,
) -> Option<ScreenedStock> {
    screened
        .iter()
        .find(|item| {
            !used_codes.contains(&item.stock.code)
                && item.theme_category.as_deref() == Some(category)
        })
        .cloned()
}
