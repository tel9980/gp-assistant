//! 数据集与启发式解析：数据体检、历史区间切片、mock 历史、指标计算与自然语言条件抽取。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use dataset::*;` 保证调用点不变。
use crate::*;

pub fn validate_data_set(data: &CoreDataSet) -> CoreResult<DataSourceSummary> {
    let mut warnings = Vec::new();
    let mut stock_codes = HashSet::new();
    for stock in &data.stocks {
        if stock.code.trim().is_empty() {
            return Err(CoreError::new("Stock code cannot be empty"));
        }
        if !stock_codes.insert(stock.code.clone()) {
            return Err(CoreError::new(format!(
                "Duplicate stock code: {}",
                stock.code
            )));
        }
    }

    for relation in &data.relations {
        if !stock_codes.contains(&relation.source_code) {
            warnings.push(format!(
                "Relation source code is not in stock universe: {}",
                relation.source_code
            ));
        }
        if !stock_codes.contains(&relation.target_code) {
            warnings.push(format!(
                "Relation target code is not in stock universe: {}",
                relation.target_code
            ));
        }
        if !(0.0..=1.0).contains(&relation.weight) {
            return Err(CoreError::new(format!(
                "Relation weight must be within [0, 1]: {} -> {}",
                relation.source_code, relation.target_code
            )));
        }
    }

    let mut history_bar_count = 0;
    for (code, bars) in &data.histories {
        if !stock_codes.contains(code) {
            warnings.push(format!("History code is not in stock universe: {code}"));
        }
        let mut previous_date = None;
        for bar in bars {
            let date = parse_date(&bar.date)?;
            if bar.close < 0.0 {
                return Err(CoreError::new(format!(
                    "History close cannot be negative: {code} {}",
                    bar.date
                )));
            }
            for (label, value) in [
                ("open", bar.open),
                ("high", bar.high),
                ("low", bar.low),
                ("volume", bar.volume),
                ("capital", bar.capital),
            ] {
                if value.map(|value| value < 0.0).unwrap_or(false) {
                    return Err(CoreError::new(format!(
                        "History {label} cannot be negative: {code} {}",
                        bar.date
                    )));
                }
            }
            if previous_date
                .map(|previous| date < previous)
                .unwrap_or(false)
            {
                warnings.push(format!("History is not sorted by date: {code}"));
            }
            previous_date = Some(date);
            history_bar_count += 1;
        }
    }

    Ok(DataSourceSummary {
        stock_count: data.stocks.len(),
        relation_count: data.relations.len(),
        history_symbol_count: data.histories.len(),
        history_bar_count,
        factor_snapshot_symbol_count: data.factor_snapshots.len(),
        factor_snapshot_count: data.factor_snapshots.values().map(Vec::len).sum(),
        warnings,
    })
}

pub fn history_bars_in_range(
    history: &[HistoryBar],
    start_date: &str,
    end_date: &str,
) -> CoreResult<Vec<HistoryBar>> {
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    if start > end {
        return Ok(Vec::new());
    }

    let mut filtered = Vec::new();
    for bar in history {
        let date = parse_date(&bar.date)?;
        if date >= start && date <= end {
            filtered.push(bar.clone());
        }
    }
    filtered.sort_by(|left, right| {
        parse_date(&left.date)
            .unwrap_or(NaiveDate::MIN)
            .cmp(&parse_date(&right.date).unwrap_or(NaiveDate::MIN))
    });
    Ok(filtered)
}

pub fn history_points_from_bars(history: &[HistoryBar]) -> CoreResult<Vec<HistoryPoint>> {
    let mut points = Vec::with_capacity(history.len());
    for bar in history {
        points.push(HistoryPoint {
            date: parse_date(&bar.date)?,
            close: bar.close,
        });
    }
    points.sort_by_key(|left| left.date);
    Ok(points)
}

pub fn mock_history(
    universe: &[StockItem],
    code: &str,
    start_date: &str,
    end_date: &str,
) -> CoreResult<Vec<HistoryBar>> {
    let stock = universe
        .iter()
        .find(|stock| stock.code == code)
        .ok_or_else(|| CoreError::new(format!("Stock {code} not found")))?;
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    if start > end {
        return Ok(Vec::new());
    }

    let mut points = Vec::new();
    let mut current = start;
    let seed: u32 = code.bytes().map(u32::from).sum();
    let drift = 0.00028 + (seed % 7) as f64 * 0.00006;
    let phase = (seed % 17) as f64 / 3.0;
    let mut previous_close = stock.price;
    let capital = stock
        .market_cap_billion
        .map(|market_cap| market_cap * 1_000_000_000.0 / stock.price.max(0.01));
    while current <= end {
        if !matches!(current.weekday(), Weekday::Sat | Weekday::Sun) {
            let index = points.len() as f64;
            let wave = (index / 8.0 + phase).sin() * 0.018;
            let pullback = (index / 23.0 + phase).sin() * 0.008;
            let close = (stock.price * (1.0 + drift * index + wave + pullback)).max(0.01);
            let open = previous_close * (1.0 + (index / 9.0 + phase).cos() * 0.004);
            let high = open.max(close) * (1.006 + (index / 11.0 + phase).sin().abs() * 0.006);
            let low = open.min(close) * (0.994 - (index / 13.0 + phase).cos().abs() * 0.004);
            let volume = (2_000_000.0 + (seed % 31) as f64 * 55_000.0 + index * 2_500.0)
                * (1.0 + (index / 10.0 + phase).sin().abs() * 0.35);
            points.push(HistoryBar {
                date: current.format("%Y-%m-%d").to_string(),
                open: Some(open),
                high: Some(high),
                low: Some(low),
                close,
                volume: Some(volume),
                capital,
            });
            previous_close = close;
        }
        current = current
            .checked_add_days(Days::new(1))
            .ok_or_else(|| CoreError::new("Date overflow"))?;
    }
    Ok(points)
}

pub fn parse_date(input: &str) -> CoreResult<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y%m%d")
        .or_else(|_| NaiveDate::parse_from_str(input, "%Y-%m-%d"))
        .map_err(|_| CoreError::new(format!("Invalid date: {input}")))
}

pub fn backtest_metrics(
    equity_curve: &[EquityPoint],
    initial_cash: f64,
) -> (f64, Option<f64>, Option<f64>) {
    let Some(first) = equity_curve.first() else {
        return (0.0, None, None);
    };
    let Some(last) = equity_curve.last() else {
        return (0.0, None, None);
    };

    let denominator = if initial_cash.is_finite() && initial_cash > 0.0 {
        initial_cash
    } else {
        first.equity
    };
    let total_return = last.equity / denominator - 1.0;
    let first_date = match NaiveDate::parse_from_str(&first.date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return (total_return, None, None),
    };
    let last_date = match NaiveDate::parse_from_str(&last.date, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return (total_return, None, None),
    };
    let days = (last_date - first_date).num_days().max(1) as f64;
    let years = days / 365.0;
    let annualized_return = if years > 0.0 {
        Some((1.0 + total_return).powf(1.0 / years) - 1.0)
    } else {
        None
    };

    let mut rolling_max = first.equity;
    let mut max_drawdown = 0.0_f64;
    for point in equity_curve {
        rolling_max = rolling_max.max(point.equity);
        if rolling_max > 0.0 {
            max_drawdown = max_drawdown.min(point.equity / rolling_max - 1.0);
        }
    }

    (total_return, annualized_return, Some(max_drawdown))
}

pub fn heuristic_criteria(message: &str) -> ScreenCriteria {
    ScreenCriteria {
        max_pe: extract_number_after(message, &["PE", "pe", "市盈率"]),
        max_pb: extract_number_after(message, &["PB", "pb", "市净率"]),
        min_roe: extract_percent_after(message, &["ROE", "roe", "净资产收益率"]),
        industry: extract_industry(message),
        ..ScreenCriteria::default()
    }
}

pub fn extract_number_after(message: &str, labels: &[&str]) -> Option<f64> {
    for label in labels {
        let pattern = format!(
            r"{}\s*(?:低于|小于|<=|<|不高于|少于|在)?\s*(\d+(?:\.\d+)?)",
            regex::escape(label)
        );
        if let Ok(regex) = Regex::new(&pattern) {
            if let Some(match_result) = regex.captures(message) {
                return match_result.get(1)?.as_str().parse().ok();
            }
        }
    }
    None
}

pub fn extract_percent_after(message: &str, labels: &[&str]) -> Option<f64> {
    for label in labels {
        let pattern = format!(
            r"{}\s*(?:高于|大于|>=|>|不低于|超过|在)?\s*(\d+(?:\.\d+)?)\s*(%)?",
            regex::escape(label)
        );
        if let Ok(regex) = Regex::new(&pattern) {
            if let Some(match_result) = regex.captures(message) {
                let value: f64 = match_result.get(1)?.as_str().parse().ok()?;
                return Some(if match_result.get(2).is_some() {
                    value / 100.0
                } else {
                    value
                });
            }
        }
    }
    None
}

pub fn extract_industry(message: &str) -> Option<String> {
    let known = [
        ("银行", "银行"),
        ("白酒", "白酒"),
        ("饮料", "白酒"),
        ("食品饮料", "食品饮料"),
        ("电池", "动力电池"),
        ("动力电池", "动力电池"),
        ("新能源", "动力电池"),
        ("汽车", "汽车"),
        ("电子", "电子制造"),
        ("半导体", "电子制造"),
        ("芯片", "电子制造"),
        ("电子制造", "电子制造"),
        ("光伏", "光伏"),
        ("化工", "化工"),
    ];
    known
        .iter()
        .find(|(keyword, _industry)| message.contains(keyword))
        .map(|(_keyword, industry)| (*industry).to_string())
}

pub fn extract_codes(message: &str) -> Vec<String> {
    let Ok(regex) = Regex::new(r"\b(\d{6})(?:\.(SH|SZ|BJ))?\b") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    regex
        .captures_iter(&message.to_uppercase())
        .filter_map(|captures| {
            let code = captures.get(1)?.as_str();
            let market = captures.get(2).map(|m| m.as_str()).unwrap_or_else(|| {
                if code.starts_with('6') {
                    "SH"
                } else if code.starts_with('8') || code.starts_with('4') {
                    "BJ"
                } else {
                    "SZ"
                }
            });
            let normalized = format!("{code}.{market}");
            if seen.insert(normalized.clone()) {
                Some(normalized)
            } else {
                None
            }
        })
        .collect()
}

pub fn extract_date(message: &str, default: &str, first: bool) -> String {
    let Ok(date_regex) = Regex::new(r"\b(20\d{2})(?:[-/.年])?(\d{1,2})(?:[-/.月])?(\d{1,2})日?\b")
    else {
        return default.to_string();
    };
    let dates: Vec<(String, String, String)> = date_regex
        .captures_iter(message)
        .filter_map(|captures| {
            Some((
                captures.get(1)?.as_str().to_string(),
                captures.get(2)?.as_str().to_string(),
                captures.get(3)?.as_str().to_string(),
            ))
        })
        .collect();
    if let Some((year, month, day)) = if first { dates.first() } else { dates.last() } {
        return format!("{year}{:0>2}{:0>2}", month, day);
    }

    let Ok(year_regex) = Regex::new(r"\b(20\d{2})\b") else {
        return default.to_string();
    };
    let years: Vec<String> = year_regex
        .captures_iter(message)
        .filter_map(|captures| captures.get(1).map(|matched| matched.as_str().to_string()))
        .collect();
    if let Some(year) = if first { years.first() } else { years.last() } {
        if first {
            format!("{year}0101")
        } else {
            format!("{year}1231")
        }
    } else {
        default.to_string()
    }
}
