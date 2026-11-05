//! 回测：数据需求解析、组合模拟（等权/走查/自适应）、因子快照、基准曲线与折损评估。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use backtest::*;` 保证调用点不变。
use crate::*;

pub fn backtest_with_mock(request: &BacktestRequest) -> CoreResult<BacktestResult> {
    backtest_with_source(&MockDataSource, request)
}

pub fn backtest_with_data(
    data: &CoreDataSet,
    request: &BacktestRequest,
) -> CoreResult<BacktestResult> {
    let source = StaticDataSource::new(data);
    backtest_with_source(&source, request)
}

pub fn backtest_selected_symbols(universe: &[StockItem], request: &BacktestRequest) -> Vec<String> {
    if is_adaptive_backtest(request) {
        return universe
            .iter()
            .filter(|stock| matches_stock(stock, &request.criteria).is_some())
            .map(|stock| stock.code.clone())
            .collect();
    }
    selected_backtest_items_from_universe(universe, request)
        .0
        .into_iter()
        .map(|item| item.stock.code)
        .collect()
}

pub struct BacktestDataRequirements {
    universe: Vec<StockItem>,
    selection_notes: Vec<String>,
    symbols: Vec<String>,
    history_symbols: Vec<String>,
}

pub fn resolve_backtest_data_requirements(
    source: &impl MarketDataSource,
    request: &BacktestRequest,
) -> CoreResult<BacktestDataRequirements> {
    validate_backtest_date_range(request)?;
    backtest_scoring_parameters(request)?;
    let mut universe = source.stocks()?.to_vec();
    let strategy_mode = backtest_strategy_mode(request);
    let dynamic_mode = strategy_mode != "candidate_snapshot";
    let source_mode = normalize_backtest_source(&request.source);
    if dynamic_mode && source_mode != "watchlist" {
        let mut known_codes = universe
            .iter()
            .map(|stock| stock.code.to_uppercase())
            .collect::<HashSet<_>>();
        for code in source.factor_snapshot_codes()? {
            let normalized = normalize_stock_code(&code);
            if normalized.is_empty() || !known_codes.insert(normalized.to_uppercase()) {
                continue;
            }
            universe.push(StockItem {
                code: normalized,
                ..StockItem::default()
            });
        }
    }
    let (selected, selection_notes) = if dynamic_mode && source_mode != "watchlist" {
        (Vec::new(), Vec::new())
    } else {
        selected_backtest_items(&universe, request)
    };
    let symbols: Vec<String> = if dynamic_mode && source_mode != "watchlist" {
        universe.iter().map(|item| item.code.clone()).collect()
    } else {
        selected
            .iter()
            .map(|item| item.stock.code.clone())
            .collect()
    };
    let history_symbols = if dynamic_mode {
        strict_walk_forward_history_symbols(
            source,
            &symbols,
            &request.start_date,
            &request.end_date,
        )?
    } else {
        symbols.clone()
    };
    Ok(BacktestDataRequirements {
        universe,
        selection_notes,
        symbols,
        history_symbols,
    })
}

pub fn validate_backtest_date_range(request: &BacktestRequest) -> CoreResult<()> {
    let start = parse_date(&request.start_date)?;
    let end = parse_date(&request.end_date)?;
    if start > end {
        return Err(CoreError::new(format!(
            "回测开始日期不能晚于结束日期：{} > {}",
            request.start_date, request.end_date
        )));
    }
    Ok(())
}

pub fn backtest_required_history_symbols(
    source: &impl MarketDataSource,
    request: &BacktestRequest,
) -> CoreResult<Vec<String>> {
    Ok(resolve_backtest_data_requirements(source, request)?.history_symbols)
}

pub fn backtest_with_source(
    source: &impl MarketDataSource,
    request: &BacktestRequest,
) -> CoreResult<BacktestResult> {
    backtest_scoring_parameters(request)?;
    let strategy_mode = backtest_strategy_mode(request);
    let dynamic_mode = strategy_mode != "candidate_snapshot";
    let BacktestDataRequirements {
        universe,
        selection_notes,
        symbols,
        history_symbols,
    } = resolve_backtest_data_requirements(source, request)?;
    let history_load_start = if is_adaptive_backtest(request) {
        "19900101"
    } else {
        &request.start_date
    };
    let histories = load_backtest_histories(
        source,
        &history_symbols,
        history_load_start,
        &request.end_date,
    )?;
    if dynamic_mode {
        let available_codes = histories
            .iter()
            .map(|history| history.code.to_uppercase())
            .collect::<HashSet<_>>();
        let missing_history_codes = history_symbols
            .iter()
            .filter(|code| !available_codes.contains(&code.to_uppercase()))
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        if !missing_history_codes.is_empty() {
            return Err(CoreError::new(format!(
                "walk_forward 严格回测缺少当期上市候选的历史行情：{}",
                missing_history_codes.join("、")
            )));
        }
    }
    if histories.is_empty() {
        if !symbols.is_empty() {
            return Err(CoreError::new(format!(
                "回测区间缺少已选 {} 只股票的历史日线，无法计算净值曲线。请联网重试或先刷新行情缓存。",
                symbols.len()
            )));
        }
        let reported_symbols = if dynamic_mode { Vec::new() } else { symbols };
        return Ok(BacktestResult {
            metrics: empty_backtest_metrics(reported_symbols.len(), &strategy_mode),
            equity_curve: Vec::new(),
            benchmark_curve: Vec::new(),
            symbols: reported_symbols,
            benchmark_symbols: Vec::new(),
            rebalance_dates: Vec::new(),
            walk_forward_folds: Vec::new(),
            volatility_snapshots: Vec::new(),
            volatility_message: Some("回测区间没有可用于波动率计算的标的日线。".to_string()),
            strategy_mode,
            notes: selection_notes,
        });
    }

    let initial_cash = sanitize_initial_cash(request.initial_cash);
    let cost_rate = sanitize_transaction_cost_rate(request.transaction_cost_bps);
    let rebalance_frequency = normalize_rebalance_frequency(&request.rebalance_frequency);
    let simulation = match strategy_mode.as_str() {
        "walk_forward" | "adaptive_swing_v1" | "adaptive_swing_v2" => {
            simulate_walk_forward_portfolio(
                source,
                &universe,
                request,
                &histories,
                initial_cash,
                cost_rate,
                &rebalance_frequency,
            )?
        }
        _ => simulate_equal_weight_portfolio(
            &histories,
            initial_cash,
            cost_rate,
            &rebalance_frequency,
        ),
    };
    let benchmark_enabled = normalize_backtest_benchmark(&request.benchmark) != "none";
    let benchmark_curve = if benchmark_enabled {
        equal_weight_benchmark_curve(
            &histories,
            initial_cash,
            parse_date(&request.start_date).ok(),
        )
    } else {
        Vec::new()
    };

    let (total_return, annualized_return, max_drawdown) =
        backtest_metrics(&simulation.equity_curve, initial_cash);
    let (benchmark_total_return, benchmark_annualized_return, benchmark_max_drawdown) =
        if benchmark_curve.is_empty() {
            (None, None, None)
        } else {
            let (total, annualized, drawdown) = backtest_metrics(&benchmark_curve, initial_cash);
            (Some(total), annualized, drawdown)
        };
    let excess_return =
        benchmark_total_return.map(|benchmark_return| total_return - benchmark_return);
    let available_symbols: Vec<String> = histories
        .iter()
        .map(|history| history.code.clone())
        .collect();
    let walk_forward_folds = if dynamic_mode {
        evaluate_walk_forward_folds(&histories, &simulation.selections)
    } else {
        Vec::new()
    };
    let reported_symbols = if dynamic_mode {
        selected_symbols_from_rebalances(&histories, &simulation.selections)
    } else {
        available_symbols.clone()
    };
    let volatility_symbols = if dynamic_mode {
        selected_symbols_from_latest_rebalance(&histories, &simulation.selections)
    } else {
        reported_symbols.clone()
    };
    let evaluated_selection_count = walk_forward_folds
        .iter()
        .map(|fold| fold.evaluated_selection_count)
        .sum::<usize>();
    let selection_hit_count = walk_forward_folds
        .iter()
        .map(|fold| fold.hit_count)
        .sum::<usize>();
    let precision_at_n = (evaluated_selection_count > 0)
        .then_some(selection_hit_count as f64 / evaluated_selection_count as f64);
    let oos_fold_count = walk_forward_folds
        .iter()
        .filter(|fold| fold.precision_at_n.is_some())
        .count();

    let mut notes = selection_notes;
    let missing_history_count = symbols.len().saturating_sub(available_symbols.len());
    if missing_history_count > 0 {
        notes.push(format!(
            "{} 只标的缺少有效历史行情，已从回测组合中剔除。",
            missing_history_count
        ));
    }
    notes.push(format!(
        "回测采用{}等权组合，交易成本为{:.1}bps。",
        backtest_rebalance_note(&rebalance_frequency),
        request.transaction_cost_bps.clamp(0.0, 500.0)
    ));
    notes.push(backtest_strategy_mode_note(&strategy_mode).to_string());
    if dynamic_mode {
        notes.push(format!(
            "滚动样本外评估共 {} 折，{} 个入选样本可计算相对候选池基准的命中率。",
            oos_fold_count, evaluated_selection_count
        ));
    }
    if benchmark_enabled {
        notes.push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
    }
    let history_by_code = histories
        .iter()
        .map(|history| (history.code.to_uppercase(), history))
        .collect::<HashMap<_, _>>();
    let stock_name_by_code = universe
        .iter()
        .filter_map(|stock| {
            let name = stock.name.trim();
            (!name.is_empty()).then(|| (stock.code.to_uppercase(), name.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let volatility_snapshots = volatility_symbols
        .iter()
        .filter_map(|symbol| {
            history_by_code
                .get(&symbol.to_uppercase())
                .and_then(|history| {
                    volatility::calculate_volatility_snapshot(symbol, &history.bars).map(
                        |mut snapshot| {
                            snapshot.name = stock_name_by_code.get(&symbol.to_uppercase()).cloned();
                            snapshot
                        },
                    )
                })
        })
        .collect::<Vec<_>>();
    let volatility_message = volatility_snapshots.is_empty().then(|| {
        if dynamic_mode && volatility_symbols.is_empty() {
            "Walk-forward 末次调仓没有符合条件的标的。".to_string()
        } else if volatility_symbols.is_empty() {
            "候选快照没有符合条件的标的。".to_string()
        } else {
            "区间末标的没有可解析的日线日期。".to_string()
        }
    });
    notes.push(
        "波动率快照按区间末可见日线计算：ATR14、布林20/2、唐奇安20、凯尔特纳EMA20+2×ATR10、Chaikin 10/10、RVI14；历史不足的指标保持缺失。"
            .to_string(),
    );

    Ok(BacktestResult {
        metrics: BacktestMetrics {
            total_return,
            annualized_return,
            max_drawdown,
            num_stocks: reported_symbols.len(),
            benchmark_total_return,
            benchmark_annualized_return,
            benchmark_max_drawdown,
            excess_return,
            total_transaction_cost: simulation.total_transaction_cost,
            total_turnover: simulation.total_turnover,
            rebalance_count: simulation.rebalance_dates.len(),
            oos_fold_count,
            evaluated_selection_count,
            selection_hit_count,
            precision_at_n,
            strategy_mode: strategy_mode.clone(),
        },
        equity_curve: simulation.equity_curve,
        benchmark_curve,
        symbols: reported_symbols,
        benchmark_symbols: if benchmark_enabled {
            available_symbols
        } else {
            Vec::new()
        },
        rebalance_dates: simulation
            .rebalance_dates
            .iter()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .collect(),
        walk_forward_folds,
        volatility_snapshots,
        volatility_message,
        strategy_mode,
        notes,
    })
}
#[derive(Clone, Debug)]
pub struct BacktestHistory {
    pub(crate) code: String,
    pub(crate) prices: BTreeMap<NaiveDate, f64>,
    pub(crate) bars: Vec<HistoryBar>,
}

#[derive(Clone, Debug)]
pub struct PortfolioSimulation {
    pub(crate) equity_curve: Vec<EquityPoint>,
    pub(crate) rebalance_dates: Vec<NaiveDate>,
    pub(crate) total_transaction_cost: f64,
    pub(crate) total_turnover: f64,
    pub(crate) selections: Vec<RebalanceSelection>,
}

#[derive(Clone, Debug)]
pub struct ActiveSelection {
    pub(crate) selected_indices: Vec<usize>,
    pub(crate) eligible_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct RebalanceSelection {
    pub(crate) signal_date: NaiveDate,
    pub(crate) date: NaiveDate,
    pub(crate) selected_indices: Vec<usize>,
    pub(crate) eligible_indices: Vec<usize>,
}

pub fn empty_backtest_metrics(num_stocks: usize, strategy_mode: &str) -> BacktestMetrics {
    BacktestMetrics {
        total_return: 0.0,
        annualized_return: None,
        max_drawdown: None,
        num_stocks,
        benchmark_total_return: None,
        benchmark_annualized_return: None,
        benchmark_max_drawdown: None,
        excess_return: None,
        total_transaction_cost: 0.0,
        total_turnover: 0.0,
        rebalance_count: 0,
        oos_fold_count: 0,
        evaluated_selection_count: 0,
        selection_hit_count: 0,
        precision_at_n: None,
        strategy_mode: strategy_mode.to_string(),
    }
}

pub fn strict_walk_forward_history_symbols(
    source: &impl MarketDataSource,
    symbols: &[String],
    start_date: &str,
    end_date: &str,
) -> CoreResult<Vec<String>> {
    let start = parse_date(start_date)?;
    let end = parse_date(end_date)?;
    let mut required = Vec::new();
    let mut missing_snapshots = Vec::new();

    for code in symbols {
        let timeline = load_factor_snapshot_timeline(source, code)?;
        if timeline.is_empty() {
            missing_snapshots.push(code.clone());
            continue;
        }
        let tradable_at_start = timeline
            .range(..=start)
            .next_back()
            .map(|(_, snapshot)| {
                snapshot.is_listed == Some(true) && snapshot.is_tradable == Some(true)
            })
            .unwrap_or(false);
        let becomes_tradable_in_range = timeline.range(start..=end).any(|(_, snapshot)| {
            snapshot.is_listed == Some(true) && snapshot.is_tradable == Some(true)
        });
        if tradable_at_start || becomes_tradable_in_range {
            required.push(code.clone());
        }
    }

    if !missing_snapshots.is_empty() {
        let preview = missing_snapshots
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        return Err(CoreError::new(format!(
            "walk_forward 严格回测要求完整的历史因子快照 factor_snapshots；缺少 {} 个：{}",
            missing_snapshots.len(),
            preview
        )));
    }

    Ok(required)
}

pub fn load_backtest_histories(
    source: &impl MarketDataSource,
    symbols: &[String],
    start_date: &str,
    end_date: &str,
) -> CoreResult<Vec<BacktestHistory>> {
    let mut histories = Vec::new();
    for code in symbols {
        let bars = source.get_history(code, start_date, end_date)?;
        let prices: BTreeMap<NaiveDate, f64> = history_points_from_bars(&bars)?
            .into_iter()
            .filter(|point| point.close.is_finite() && point.close > 0.0)
            .map(|point| (point.date, point.close))
            .collect();
        if !prices.is_empty() {
            histories.push(BacktestHistory {
                code: code.clone(),
                prices,
                bars,
            });
        }
    }
    Ok(histories)
}

pub fn simulate_equal_weight_portfolio(
    histories: &[BacktestHistory],
    initial_cash: f64,
    transaction_cost_rate: f64,
    rebalance_frequency: &str,
) -> PortfolioSimulation {
    let dates = backtest_calendar(histories);
    let mut cash = initial_cash;
    let mut holdings = vec![0.0; histories.len()];
    let mut last_rebalance_date: Option<NaiveDate> = None;
    let mut equity_curve = Vec::with_capacity(dates.len());
    let mut rebalance_dates = Vec::new();
    let mut total_transaction_cost = 0.0;
    let mut total_turnover = 0.0;

    for date in dates {
        let current_prices: Vec<Option<f64>> = histories
            .iter()
            .map(|history| latest_price_on_or_before(history, date))
            .collect();
        let equity_before_rebalance = cash
            + holdings
                .iter()
                .zip(current_prices.iter())
                .map(|(shares, price)| shares * price.unwrap_or(0.0))
                .sum::<f64>();

        if should_rebalance(date, last_rebalance_date, rebalance_frequency)
            && equity_before_rebalance > 0.0
        {
            let active_indices: Vec<usize> = current_prices
                .iter()
                .enumerate()
                .filter_map(|(index, price)| price.filter(|value| *value > 0.0).map(|_| index))
                .collect();
            if !active_indices.is_empty() {
                let target_weight = 1.0 / active_indices.len() as f64;
                let mut turnover_value = 0.0;
                for index in 0..histories.len() {
                    let price = current_prices[index].unwrap_or(0.0);
                    let target_value = if price > 0.0 && active_indices.contains(&index) {
                        equity_before_rebalance * target_weight
                    } else {
                        0.0
                    };
                    let current_value = holdings[index] * price;
                    turnover_value += (target_value - current_value).abs();
                }

                let transaction_cost = turnover_value * transaction_cost_rate;
                let investable_equity = (equity_before_rebalance - transaction_cost).max(0.0);
                cash = investable_equity;
                holdings.fill(0.0);
                for index in active_indices {
                    if let Some(price) = current_prices[index].filter(|value| *value > 0.0) {
                        let target_value = investable_equity * target_weight;
                        holdings[index] = target_value / price;
                        cash -= target_value;
                    }
                }
                total_transaction_cost += transaction_cost;
                total_turnover += turnover_value / equity_before_rebalance;
                rebalance_dates.push(date);
                last_rebalance_date = Some(date);
            }
        }

        let equity = cash
            + holdings
                .iter()
                .zip(current_prices.iter())
                .map(|(shares, price)| shares * price.unwrap_or(0.0))
                .sum::<f64>();
        equity_curve.push(EquityPoint {
            date: date.format("%Y-%m-%d").to_string(),
            equity,
        });
    }

    PortfolioSimulation {
        equity_curve,
        rebalance_dates,
        total_transaction_cost,
        total_turnover,
        selections: Vec::new(),
    }
}

pub fn simulate_walk_forward_portfolio(
    source: &impl MarketDataSource,
    universe: &[StockItem],
    request: &BacktestRequest,
    histories: &[BacktestHistory],
    initial_cash: f64,
    transaction_cost_rate: f64,
    rebalance_frequency: &str,
) -> CoreResult<PortfolioSimulation> {
    let start_date = parse_date(&request.start_date)?;
    let end_date = parse_date(&request.end_date)?;
    let dates = backtest_calendar(histories)
        .into_iter()
        .filter(|date| *date >= start_date && *date <= end_date)
        .collect::<Vec<_>>();
    let mut snapshots_by_code = HashMap::new();
    let mut missing_snapshot_codes = Vec::new();
    for history in histories {
        let timeline = load_factor_snapshot_timeline(source, &history.code)?;
        if timeline.is_empty() {
            missing_snapshot_codes.push(history.code.clone());
        }
        snapshots_by_code.insert(history.code.clone(), timeline);
    }
    if !missing_snapshot_codes.is_empty() {
        let preview = missing_snapshot_codes
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("、");
        return Err(CoreError::new(format!(
            "walk_forward 严格回测要求每个有历史行情的标的都有历史因子快照 factor_snapshots；缺少 {} 个：{}",
            missing_snapshot_codes.len(),
            preview
        )));
    }
    let benchmark_histories = if is_adaptive_backtest(request) {
        let histories = ["000001.SH", "399001.SZ", "399006.SZ"]
            .into_iter()
            .filter_map(|code| {
                source
                    .get_history(code, "19900101", &request.end_date)
                    .ok()
                    .filter(|bars| bars.len() >= MIN_HISTORY_BARS_FOR_ADAPTIVE_BACKTEST)
                    .map(|bars| (code.to_string(), bars))
            })
            .collect::<HashMap<_, _>>();
        if adaptive_backtest_requested_mode(&request.strategy_mode) == "auto" && histories.len() < 2
        {
            return Err(CoreError::new(
                "adaptive_swing_v1 回测至少需要2个有效宽基指数的历史日线",
            ));
        }
        histories
    } else {
        HashMap::new()
    };
    simulate_walk_forward_portfolio_with_snapshots(
        universe,
        request,
        histories,
        &snapshots_by_code,
        dates,
        initial_cash,
        transaction_cost_rate,
        rebalance_frequency,
        &benchmark_histories,
    )
}

pub const MIN_HISTORY_BARS_FOR_ADAPTIVE_BACKTEST: usize = 60;

#[allow(clippy::too_many_arguments)] // 内部模拟函数，参数即上下文，拆结构体反而更难读
pub fn simulate_walk_forward_portfolio_with_snapshots(
    universe: &[StockItem],
    request: &BacktestRequest,
    histories: &[BacktestHistory],
    snapshots_by_code: &HashMap<String, BTreeMap<NaiveDate, StockFactorSnapshot>>,
    dates: Vec<NaiveDate>,
    initial_cash: f64,
    transaction_cost_rate: f64,
    rebalance_frequency: &str,
    benchmark_histories: &HashMap<String, Vec<HistoryBar>>,
) -> CoreResult<PortfolioSimulation> {
    backtest_scoring_parameters(request)?;
    let mut cash = initial_cash;
    let mut holdings = vec![0.0; histories.len()];
    let mut last_rebalance_date: Option<NaiveDate> = None;
    let mut previous_trade_date: Option<NaiveDate> = None;
    let mut equity_curve = Vec::with_capacity(dates.len());
    let mut rebalance_dates = Vec::new();
    let mut total_transaction_cost = 0.0;
    let mut total_turnover = 0.0;
    let mut selections = Vec::new();
    let history_index: HashMap<String, usize> = histories
        .iter()
        .enumerate()
        .map(|(index, history)| (history.code.to_uppercase(), index))
        .collect();

    for date in dates {
        let valuation_prices: Vec<Option<f64>> = histories
            .iter()
            .map(|history| latest_price_on_or_before(history, date))
            .collect();
        let trade_prices: Vec<Option<f64>> = histories
            .iter()
            .map(|history| price_on_date(history, date))
            .collect();
        let equity_before_rebalance = cash
            + holdings
                .iter()
                .zip(valuation_prices.iter())
                .map(|(shares, price)| shares * price.unwrap_or(0.0))
                .sum::<f64>();

        if should_rebalance(date, last_rebalance_date, rebalance_frequency)
            && equity_before_rebalance > 0.0
            && previous_trade_date.is_some()
        {
            let Some(signal_date) = previous_trade_date else {
                unreachable!("checked above");
            };
            let signal_prices = histories
                .iter()
                .map(|history| price_on_date(history, signal_date))
                .collect::<Vec<_>>();
            let active = if is_adaptive_backtest(request) {
                adaptive_walk_forward_active_indices(
                    universe,
                    request,
                    histories,
                    benchmark_histories,
                    snapshots_by_code,
                    &history_index,
                    &signal_prices,
                    signal_date,
                )?
            } else {
                walk_forward_active_indices(
                    universe,
                    request,
                    snapshots_by_code,
                    &history_index,
                    &signal_prices,
                    signal_date,
                )
            };
            let target_weight = if active.selected_indices.is_empty() {
                0.0
            } else {
                1.0 / active.selected_indices.len() as f64
            };
            let active_set: HashSet<usize> = active.selected_indices.iter().copied().collect();
            let tradable_equity_before = cash
                + holdings
                    .iter()
                    .zip(trade_prices.iter())
                    .map(|(shares, price)| shares * price.unwrap_or(0.0))
                    .sum::<f64>();
            let mut turnover_value = 0.0;
            for index in 0..histories.len() {
                let price = trade_prices[index].unwrap_or(0.0);
                let target_value = if price > 0.0 && active_set.contains(&index) {
                    tradable_equity_before * target_weight
                } else {
                    0.0
                };
                if price > 0.0 {
                    let current_value = holdings[index] * price;
                    turnover_value += (target_value - current_value).abs();
                }
            }

            let transaction_cost = turnover_value * transaction_cost_rate;
            let investable_equity = (tradable_equity_before - transaction_cost).max(0.0);
            cash = investable_equity;
            for (index, price) in trade_prices.iter().enumerate() {
                if price.is_some() {
                    holdings[index] = 0.0;
                }
            }
            for index in active.selected_indices.iter().copied() {
                if let Some(price) = trade_prices[index].filter(|value| *value > 0.0) {
                    let target_value = investable_equity * target_weight;
                    holdings[index] = target_value / price;
                    cash -= target_value;
                }
            }
            total_transaction_cost += transaction_cost;
            total_turnover += turnover_value / equity_before_rebalance;
            rebalance_dates.push(date);
            selections.push(RebalanceSelection {
                signal_date,
                date,
                selected_indices: active.selected_indices,
                eligible_indices: active.eligible_indices,
            });
            last_rebalance_date = Some(date);
        }

        let equity = cash
            + holdings
                .iter()
                .zip(valuation_prices.iter())
                .map(|(shares, price)| shares * price.unwrap_or(0.0))
                .sum::<f64>();
        equity_curve.push(EquityPoint {
            date: date.format("%Y-%m-%d").to_string(),
            equity,
        });
        previous_trade_date = Some(date);
    }

    Ok(PortfolioSimulation {
        equity_curve,
        rebalance_dates,
        total_transaction_cost,
        total_turnover,
        selections,
    })
}

pub fn load_factor_snapshot_timeline(
    source: &impl MarketDataSource,
    code: &str,
) -> CoreResult<BTreeMap<NaiveDate, StockFactorSnapshot>> {
    let mut timeline = BTreeMap::new();
    for snapshot in source.get_factor_snapshots(code)? {
        let available_date = snapshot.available_date.as_deref().ok_or_else(|| {
            CoreError::new(format!(
                "历史因子快照缺少 available_date，无法判断数据何时可用：{code} {}",
                snapshot.date
            ))
        })?;
        let report_date = parse_date(&snapshot.date).map_err(|_| {
            CoreError::new(format!(
                "历史因子快照报告期 date 无效：{code} {}",
                snapshot.date
            ))
        })?;
        if snapshot.is_st.is_none()
            || snapshot.is_listed.is_none()
            || snapshot.is_tradable.is_none()
        {
            return Err(CoreError::new(format!(
                "历史因子快照缺少 is_st/is_listed/is_tradable 状态：{code} {available_date}"
            )));
        }
        let availability_date = parse_date(available_date).map_err(|_| {
            CoreError::new(format!(
                "历史因子快照 available_date 无效：{code} {available_date}"
            ))
        })?;
        if availability_date < report_date {
            return Err(CoreError::new(format!(
                "历史因子快照 available_date 早于报告期 date：{code} {} 早于 {}",
                available_date, snapshot.date
            )));
        }
        timeline.insert(availability_date, snapshot);
    }
    Ok(timeline)
}

pub fn walk_forward_active_indices(
    universe: &[StockItem],
    request: &BacktestRequest,
    snapshots_by_code: &HashMap<String, BTreeMap<NaiveDate, StockFactorSnapshot>>,
    history_index: &HashMap<String, usize>,
    current_prices: &[Option<f64>],
    date: NaiveDate,
) -> ActiveSelection {
    if normalize_backtest_source(&request.source) == "watchlist" {
        let eligible_indices = dedupe_stock_codes(&request.stock_codes)
            .into_iter()
            .filter_map(|code| {
                let index = history_index.get(&code.to_uppercase()).copied()?;
                let snapshot = latest_factor_snapshot_on_or_before(snapshots_by_code, &code, date)?;
                if snapshot.is_listed != Some(true)
                    || snapshot.is_tradable != Some(true)
                    || (!request.criteria.include_st && snapshot.is_st == Some(true))
                {
                    return None;
                }
                current_prices
                    .get(index)
                    .and_then(|price| *price)
                    .filter(|price| price.is_finite() && *price > 0.0)?;
                Some(index)
            })
            .collect::<Vec<_>>();
        let selected_indices = eligible_indices
            .iter()
            .take(request.top_n.clamp(1, 100))
            .copied()
            .collect();
        return ActiveSelection {
            eligible_indices,
            selected_indices,
        };
    }

    let visible_universe = point_in_time_universe(
        universe,
        snapshots_by_code,
        history_index,
        current_prices,
        date,
    );
    let selected = selected_backtest_items_from_universe(&visible_universe, request).0;
    let eligible_indices = visible_universe
        .iter()
        .filter_map(|stock| history_index.get(&stock.code.to_uppercase()).copied())
        .collect::<Vec<_>>();
    let mut indices = Vec::new();
    for item in selected.into_iter().take(request.top_n.clamp(1, 100)) {
        let code = item.stock.code.to_uppercase();
        if let Some(index) = history_index.get(&code).copied() {
            if current_prices
                .get(index)
                .and_then(|price| *price)
                .filter(|price| *price > 0.0)
                .is_some()
            {
                indices.push(index);
            }
        }
    }
    ActiveSelection {
        selected_indices: indices,
        eligible_indices,
    }
}

#[allow(clippy::too_many_arguments)] // 参数即上下文，拆结构体反而更难读
pub fn adaptive_walk_forward_active_indices(
    universe: &[StockItem],
    request: &BacktestRequest,
    histories: &[BacktestHistory],
    benchmark_histories: &HashMap<String, Vec<HistoryBar>>,
    snapshots_by_code: &HashMap<String, BTreeMap<NaiveDate, StockFactorSnapshot>>,
    history_index: &HashMap<String, usize>,
    current_prices: &[Option<f64>],
    date: NaiveDate,
) -> CoreResult<ActiveSelection> {
    let mut visible_universe = point_in_time_universe(
        universe,
        snapshots_by_code,
        history_index,
        current_prices,
        date,
    );
    let eligible_indices = visible_universe
        .iter()
        .filter_map(|stock| history_index.get(&stock.code.to_uppercase()).copied())
        .collect::<Vec<_>>();
    if visible_universe.is_empty() {
        return Ok(ActiveSelection {
            eligible_indices,
            selected_indices: Vec::new(),
        });
    }

    let mut point_in_time_histories = HashMap::new();
    for stock in &mut visible_universe {
        let Some(index) = history_index.get(&stock.code.to_uppercase()).copied() else {
            continue;
        };
        let Some(history) = histories.get(index) else {
            continue;
        };
        let bars = history
            .bars
            .iter()
            .filter(|bar| {
                parse_date(&bar.date)
                    .map(|bar_date| bar_date <= date)
                    .unwrap_or(false)
            })
            .rev()
            .take(120)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        if bars.last().and_then(|bar| parse_date(&bar.date).ok()) != Some(date) {
            return Err(CoreError::new(format!(
                "adaptive_swing_v1 调仓信号日缺少股票日线：{} {}",
                stock.code,
                date.format("%Y-%m-%d")
            )));
        }
        if bars.len() >= 2 {
            let latest = bars[bars.len() - 1].close;
            let previous = bars[bars.len() - 2].close;
            stock.change_pct = (previous > 0.0).then_some(latest / previous - 1.0);
        }
        stock.quote_time = Some(date.format("%Y-%m-%d").to_string());
        point_in_time_histories.insert(stock.code.clone(), bars);
    }
    let point_in_time_benchmarks = benchmark_histories
        .iter()
        .filter_map(|(code, bars)| {
            let visible = bars
                .iter()
                .filter(|bar| {
                    parse_date(&bar.date)
                        .map(|bar_date| bar_date <= date)
                        .unwrap_or(false)
                })
                .rev()
                .take(120)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>();
            (visible.last().and_then(|bar| parse_date(&bar.date).ok()) == Some(date))
                .then(|| (code.clone(), visible))
        })
        .collect::<HashMap<_, _>>();
    let adaptive_request = AdaptiveScreenRequest {
        criteria: request.criteria.clone(),
        mode: adaptive_backtest_requested_mode(&request.strategy_mode),
        horizon: "swing_10_30d".to_string(),
        primary_limit: request.top_n.clamp(1, 100),
        exploration_limit: 0,
        run_id: None,
        as_of_date: Some(date.format("%Y%m%d").to_string()),
    };
    let parameters = backtest_scoring_parameters(request)?
        .ok_or_else(|| CoreError::new("adaptive backtest requires adaptive scoring parameters"))?;
    let selected_indices = adaptive_screen_stocks_with_parameters(
        &visible_universe,
        &point_in_time_histories,
        &point_in_time_benchmarks,
        &[],
        &adaptive_request,
        &parameters,
    )
    .map_err(|error| {
        CoreError::new(format!(
            "adaptive_swing_v1 调仓日 {} 数据不足：{}",
            date.format("%Y-%m-%d"),
            error
        ))
    })?
    .items
    .into_iter()
    .filter_map(|item| history_index.get(&item.stock.code.to_uppercase()).copied())
    .collect::<Vec<_>>();
    Ok(ActiveSelection {
        selected_indices,
        eligible_indices,
    })
}

pub fn point_in_time_universe(
    universe: &[StockItem],
    snapshots_by_code: &HashMap<String, BTreeMap<NaiveDate, StockFactorSnapshot>>,
    history_index: &HashMap<String, usize>,
    current_prices: &[Option<f64>],
    date: NaiveDate,
) -> Vec<StockItem> {
    universe
        .iter()
        .filter_map(|stock| {
            let snapshot =
                latest_factor_snapshot_on_or_before(snapshots_by_code, &stock.code, date)?;
            if snapshot.is_listed != Some(true) || snapshot.is_tradable != Some(true) {
                return None;
            }
            snapshot.is_st?;
            let index = history_index.get(&stock.code.to_uppercase()).copied()?;
            let price = current_prices
                .get(index)
                .and_then(|price| *price)
                .filter(|price| price.is_finite() && *price > 0.0)?;
            Some(apply_factor_snapshot(stock, snapshot, price))
        })
        .collect()
}

pub fn latest_factor_snapshot_on_or_before<'a>(
    snapshots_by_code: &'a HashMap<String, BTreeMap<NaiveDate, StockFactorSnapshot>>,
    code: &str,
    date: NaiveDate,
) -> Option<&'a StockFactorSnapshot> {
    snapshots_by_code
        .get(code)
        .or_else(|| snapshots_by_code.get(&code.to_uppercase()))
        .and_then(|timeline| {
            timeline
                .range(..=date)
                .next_back()
                .map(|(_, snapshot)| snapshot)
        })
}

pub fn apply_factor_snapshot(
    stock: &StockItem,
    snapshot: &StockFactorSnapshot,
    historical_price: f64,
) -> StockItem {
    let finite = |value: Option<f64>| value.filter(|value| value.is_finite());
    StockItem {
        code: stock.code.clone(),
        name: snapshot.name.clone().unwrap_or_default(),
        industry: snapshot.industry.clone().unwrap_or_default(),
        is_st: snapshot.is_st.unwrap_or(true),
        price: historical_price,
        pe: finite(snapshot.pe),
        pb: finite(snapshot.pb),
        roe: finite(snapshot.roe),
        market_cap_billion: finite(snapshot.market_cap_billion),
        dividend_yield: finite(snapshot.dividend_yield),
        latest_eps: None,
        deducted_net_profit_billion: finite(snapshot.deducted_net_profit_billion),
        deducted_net_profit_margin: finite(snapshot.deducted_net_profit_margin),
        deducted_net_profit_growth_rate: finite(snapshot.deducted_net_profit_growth_rate),
        change_pct: finite(snapshot.change_pct),
        volume: finite(snapshot.volume),
        amount: finite(snapshot.amount),
        turnover_rate: finite(snapshot.turnover_rate),
        volume_ratio: finite(snapshot.volume_ratio),
        quote_time: snapshot.available_date.clone(),
    }
}

pub fn equal_weight_benchmark_curve(
    histories: &[BacktestHistory],
    initial_cash: f64,
    start_date: Option<NaiveDate>,
) -> Vec<EquityPoint> {
    backtest_calendar(histories)
        .into_iter()
        .filter(|date| start_date.is_none_or(|start| *date >= start))
        .filter_map(|date| {
            let values: Vec<f64> = histories
                .iter()
                .filter_map(|history| {
                    let first = match start_date {
                        Some(start) => history
                            .prices
                            .range(start..)
                            .next()
                            .map(|(_, price)| *price)?,
                        None => history.prices.values().next().copied()?,
                    };
                    let price = latest_price_on_or_before(history, date)?;
                    if first > 0.0 && price > 0.0 {
                        Some(price / first)
                    } else {
                        None
                    }
                })
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(EquityPoint {
                    date: date.format("%Y-%m-%d").to_string(),
                    equity: initial_cash * values.iter().sum::<f64>() / values.len() as f64,
                })
            }
        })
        .collect()
}

pub fn backtest_calendar(histories: &[BacktestHistory]) -> Vec<NaiveDate> {
    let mut dates = BTreeMap::new();
    for history in histories {
        for date in history.prices.keys() {
            dates.insert(*date, ());
        }
    }
    dates.into_keys().collect()
}

pub fn latest_price_on_or_before(history: &BacktestHistory, date: NaiveDate) -> Option<f64> {
    history
        .prices
        .range(..=date)
        .next_back()
        .map(|(_, price)| *price)
}

pub fn price_on_date(history: &BacktestHistory, date: NaiveDate) -> Option<f64> {
    history.prices.get(&date).copied()
}

pub fn selected_symbols_from_latest_rebalance(
    histories: &[BacktestHistory],
    selections: &[RebalanceSelection],
) -> Vec<String> {
    selections
        .last()
        .map(|selection| {
            selection
                .selected_indices
                .iter()
                .filter_map(|index| histories.get(*index).map(|history| history.code.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn selected_symbols_from_rebalances(
    histories: &[BacktestHistory],
    selections: &[RebalanceSelection],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();
    for selection in selections {
        for index in &selection.selected_indices {
            let Some(history) = histories.get(*index) else {
                continue;
            };
            if seen.insert(history.code.clone()) {
                symbols.push(history.code.clone());
            }
        }
    }
    symbols
}

pub fn evaluate_walk_forward_folds(
    histories: &[BacktestHistory],
    selections: &[RebalanceSelection],
) -> Vec<WalkForwardFold> {
    let final_date = backtest_calendar(histories).last().copied();
    selections
        .iter()
        .enumerate()
        .map(|(position, selection)| {
            let evaluation_end = selections
                .get(position + 1)
                .map(|next| next.date)
                .or(final_date)
                .filter(|end| *end > selection.date);
            let selected_symbols = selection
                .selected_indices
                .iter()
                .filter_map(|index| histories.get(*index).map(|history| history.code.clone()))
                .collect::<Vec<_>>();
            let selected_returns = evaluation_end
                .map(|end| {
                    selection
                        .selected_indices
                        .iter()
                        .map(|index| {
                            histories
                                .get(*index)
                                .and_then(|history| forward_return(history, selection.date, end))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let benchmark_returns = evaluation_end
                .map(|end| {
                    selection
                        .eligible_indices
                        .iter()
                        .filter_map(|index| {
                            forward_return(histories.get(*index)?, selection.date, end)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let benchmark_forward_return = mean(&benchmark_returns);
            let realized_selected_returns = selected_returns
                .iter()
                .filter_map(|value| *value)
                .collect::<Vec<_>>();
            let average_forward_return = mean(&realized_selected_returns);
            let (evaluated_selection_count, hit_count, precision_at_n) =
                if let Some(benchmark) = benchmark_forward_return {
                    let hit_count = selected_returns
                        .iter()
                        .filter_map(|value| *value)
                        .filter(|value| *value > benchmark)
                        .count();
                    let evaluated = selected_returns.len();
                    let precision = (evaluated > 0).then_some(hit_count as f64 / evaluated as f64);
                    (evaluated, hit_count, precision)
                } else {
                    (0, 0, None)
                };
            WalkForwardFold {
                signal_date: Some(selection.signal_date.format("%Y-%m-%d").to_string()),
                selection_date: selection.date.format("%Y-%m-%d").to_string(),
                evaluation_end_date: evaluation_end.map(|date| date.format("%Y-%m-%d").to_string()),
                selected_symbols,
                eligible_symbol_count: selection.eligible_indices.len(),
                evaluated_selection_count,
                hit_count,
                precision_at_n,
                average_forward_return,
                benchmark_forward_return,
                average_excess_return: average_forward_return
                    .zip(benchmark_forward_return)
                    .map(|(selected, benchmark)| selected - benchmark),
            }
        })
        .collect()
}

pub fn forward_return(history: &BacktestHistory, start: NaiveDate, end: NaiveDate) -> Option<f64> {
    let start_price = price_on_date(history, start)?;
    let end_price = price_on_date(history, end)?;
    (start_price > 0.0 && end_price > 0.0).then_some(end_price / start_price - 1.0)
}

pub fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then_some(values.iter().sum::<f64>() / values.len() as f64)
}

pub fn should_rebalance(
    date: NaiveDate,
    last_rebalance_date: Option<NaiveDate>,
    frequency: &str,
) -> bool {
    let Some(previous) = last_rebalance_date else {
        return true;
    };
    match frequency {
        "none" => false,
        "quarterly" => date.year() != previous.year() || quarter(date) != quarter(previous),
        _ => date.year() != previous.year() || date.month() != previous.month(),
    }
}

pub fn quarter(date: NaiveDate) -> u32 {
    (date.month0() / 3) + 1
}

pub fn sanitize_initial_cash(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        default_initial_cash()
    }
}

pub fn sanitize_transaction_cost_rate(bps: f64) -> f64 {
    if bps.is_finite() {
        bps.clamp(0.0, 500.0) / 10_000.0
    } else {
        default_transaction_cost_bps() / 10_000.0
    }
}

pub fn normalize_rebalance_frequency(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => "none".to_string(),
        "quarterly" => "quarterly".to_string(),
        _ => "monthly".to_string(),
    }
}

pub fn normalize_backtest_benchmark(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("none") {
        "none".to_string()
    } else {
        "candidate_equal_weight".to_string()
    }
}

pub fn normalize_backtest_strategy_mode(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.split(':').next() == Some("adaptive_swing_v2") {
        return "adaptive_swing_v2".to_string();
    }
    if normalized.split(':').next() == Some("adaptive_swing_v1") {
        return "adaptive_swing_v1".to_string();
    }
    match normalized.as_str() {
        "candidate_snapshot" | "snapshot" => "candidate_snapshot".to_string(),
        _ => "walk_forward".to_string(),
    }
}

pub fn backtest_strategy_mode(request: &BacktestRequest) -> String {
    request
        .algorithm_version
        .clone()
        .unwrap_or_else(|| normalize_backtest_strategy_mode(&request.strategy_mode))
}

pub fn is_adaptive_backtest(request: &BacktestRequest) -> bool {
    matches!(
        backtest_strategy_mode(request).as_str(),
        "adaptive_swing_v1" | "adaptive_swing_v2"
    )
}

pub fn backtest_scoring_parameters(
    request: &BacktestRequest,
) -> CoreResult<Option<ScoringParameters>> {
    let mode = backtest_strategy_mode(request);
    let declared = normalize_backtest_strategy_mode(&request.strategy_mode);
    if request.algorithm_version.is_some()
        && !matches!(mode.as_str(), "adaptive_swing_v1" | "adaptive_swing_v2")
    {
        return Err(CoreError::new("unsupported backtest algorithm_version"));
    }
    if request.algorithm_version.is_some()
        && (declared == "candidate_snapshot"
            || (declared.starts_with("adaptive_swing_") && declared != mode))
    {
        return Err(CoreError::new(
            "algorithm_version conflicts with explicit strategy_mode",
        ));
    }
    if !is_adaptive_backtest(request) {
        if request.scoring_parameters.is_some() || request.parameter_set_id.is_some() {
            return Err(CoreError::new(
                "scoring parameters require an adaptive backtest algorithm",
            ));
        }
        return Ok(None);
    }
    let parameters = match &request.scoring_parameters {
        Some(parameters) => parameters.clone(),
        None if mode == "adaptive_swing_v1" => fixed_scoring_parameters(),
        None => {
            return Err(CoreError::new(
                "adaptive_swing_v2 requires scoring parameters",
            ))
        }
    };
    validate_scoring_parameters(&parameters)?;
    if parameters.algorithm_version != mode {
        return Err(CoreError::new(
            "scoring parameters algorithm_version does not match backtest",
        ));
    }
    if request
        .parameter_set_id
        .as_ref()
        .is_some_and(|id| *id != parameters.parameter_set_id)
    {
        return Err(CoreError::new("backtest parameter_set_id mismatch"));
    }
    Ok(Some(parameters))
}

pub fn adaptive_backtest_requested_mode(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_once(':')
        .map(|(_, mode)| mode)
        .filter(|mode| matches!(*mode, "auto" | "range" | "trend" | "defensive"))
        .unwrap_or("auto")
        .to_string()
}

pub fn backtest_strategy_mode_note(value: &str) -> &'static str {
    match value {
        "adaptive_swing_v2" => "策略模式：adaptive_swing_v2；使用经校验的冻结参数及前一交易日可见数据生成信号，在下一交易日按实际报价调仓。",
        "adaptive_swing_v1" => "策略模式：adaptive_swing_v1；使用前一交易日收盘前可见的财务快照、OHLCV、三宽基指数和市场宽度生成信号，并在下一交易日按实际报价调仓。",
        "walk_forward" => "策略模式：walk_forward，仅使用成交日前一交易日已公开的因子快照和历史上市/ST/可交易状态；成交只使用执行日实际报价。",
        _ => "策略模式：candidate_snapshot，展示当前候选池的历史组合表现，不等同于严格可交易的逐日选股策略。",
    }
}

pub fn backtest_rebalance_note(value: &str) -> &'static str {
    match value {
        "none" => "买入持有",
        "quarterly" => "季度调仓",
        _ => "月度调仓",
    }
}
pub fn selected_backtest_items(
    universe: &[StockItem],
    request: &BacktestRequest,
) -> (Vec<ScreenedStock>, Vec<String>) {
    selected_backtest_items_from_universe(universe, request)
}

pub fn selected_backtest_items_from_universe(
    universe: &[StockItem],
    request: &BacktestRequest,
) -> (Vec<ScreenedStock>, Vec<String>) {
    if normalize_backtest_source(&request.source) != "watchlist" {
        let screened = screen_stocks(universe, &request.criteria);
        return (
            screened
                .items
                .into_iter()
                .take(request.top_n.clamp(1, 100))
                .collect(),
            Vec::new(),
        );
    }

    let codes = dedupe_stock_codes(&request.stock_codes);
    if codes.is_empty() {
        return (
            Vec::new(),
            vec!["自选观察池为空，未执行固定标的回测。".to_string()],
        );
    }

    let by_code: HashMap<String, StockItem> = universe
        .iter()
        .map(|stock| (stock.code.to_uppercase(), stock.clone()))
        .collect();
    let mut selected = Vec::new();
    let mut missing = Vec::new();
    for code in codes.iter().take(request.top_n.clamp(1, 100)) {
        if let Some(stock) = by_code.get(code) {
            selected.push(score_stock(stock, &["watchlist".to_string()], "quality"));
        } else {
            missing.push(code.clone());
        }
    }

    let mut notes = vec![format!(
        "回测标的来源：自选观察池，已选择 {} / {} 只。",
        selected.len(),
        codes.len()
    )];
    if !missing.is_empty() {
        let sample = missing
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let suffix = if missing.len() > 5 { " 等" } else { "" };
        notes.push(format!(
            "自选观察池中 {} 只股票不在当前股票池：{}{}。",
            missing.len(),
            sample,
            suffix
        ));
    }
    (selected, notes)
}

pub fn normalize_backtest_source(source: &str) -> String {
    if source.trim().eq_ignore_ascii_case("watchlist") {
        "watchlist".to_string()
    } else {
        "criteria".to_string()
    }
}

pub fn dedupe_stock_codes(codes: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for code in codes {
        let normalized = normalize_stock_code(code);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        result.push(normalized);
    }
    result
}

pub fn normalize_stock_code(code: &str) -> String {
    let text = code.trim().to_uppercase();
    if text.is_empty() {
        return String::new();
    }
    if let Some((digits, suffix)) = text.split_once('.') {
        if digits.chars().all(|ch| ch.is_ascii_digit()) && matches!(suffix, "SH" | "SZ" | "BJ") {
            return format!("{:0>6}.{}", digits, suffix);
        }
        return text;
    }
    let digits: String = text.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() != 6 {
        return text;
    }
    let suffix = if digits.starts_with('5') || digits.starts_with('6') || digits.starts_with('9') {
        "SH"
    } else if digits.starts_with('4') || digits.starts_with('8') {
        "BJ"
    } else {
        "SZ"
    };
    format!("{}.{}", digits, suffix)
}
