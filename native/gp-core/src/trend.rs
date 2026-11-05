//! 趋势与技术面：K 线预处理、指标计算（KDM/EMA/DMA/KDJ/MACD）、形态与分层评分、信号分类与说明。
//!
//! 从 lib.rs 抽出，逻辑未改；`lib.rs` 通过 `pub use trend::*;` 保证调用点不变。
use crate::*;

pub fn screen_candidate_pool(
    universe: &[StockItem],
    criteria: &ScreenCriteria,
    requested_limit: usize,
) -> Vec<ScreenedStock> {
    let pool_size = 200.min((requested_limit * 5).max(criteria.limit).max(50));
    let mut expanded_criteria = criteria.clone();
    expanded_criteria.limit = pool_size;
    screen_stocks(universe, &expanded_criteria).items
}

pub fn assign_weights(signals: &mut [GraphStockSignal]) {
    if signals.is_empty() {
        return;
    }

    let positive_sum: f64 = signals.iter().map(|item| item.final_score.max(0.0)).sum();
    if positive_sum <= 0.0 {
        let equal_weight = round6(1.0 / signals.len() as f64);
        for item in signals {
            item.suggested_weight = equal_weight;
        }
        return;
    }

    for item in signals {
        item.suggested_weight = round6(item.final_score.max(0.0) / positive_sum);
    }
}

pub fn prepare_bars(history: &[HistoryBar], stock: &StockItem) -> CoreResult<Vec<PreparedBar>> {
    let fallback_capital = stock
        .market_cap_billion
        .map(|market_cap| market_cap * 1_000_000_000.0 / stock.price.max(0.01))
        .unwrap_or_else(|| {
            let max_volume = history
                .iter()
                .filter_map(|bar| bar.volume)
                .fold(0.0_f64, f64::max);
            (max_volume * 20.0).max(1.0)
        });
    let mut bars = Vec::with_capacity(history.len());
    for bar in history {
        let date = parse_date(&bar.date)?;
        let open = bar.open.unwrap_or(bar.close);
        let high = bar.high.unwrap_or(open.max(bar.close));
        let low = bar.low.unwrap_or(open.min(bar.close));
        bars.push(PreparedBar {
            date,
            open,
            high,
            low,
            close: bar.close,
            volume: bar.volume.unwrap_or(0.0).max(0.0),
            capital: bar.capital.unwrap_or(fallback_capital).max(1.0),
        });
    }
    bars.sort_by_key(|left| left.date);
    Ok(bars)
}

pub fn compute_trend_bars(bars: &[PreparedBar]) -> Vec<ComputedTrendBar> {
    let close: Vec<f64> = bars.iter().map(|bar| bar.close).collect();
    let open: Vec<f64> = bars.iter().map(|bar| bar.open).collect();
    let high: Vec<f64> = bars.iter().map(|bar| bar.high).collect();
    let low: Vec<f64> = bars.iter().map(|bar| bar.low).collect();
    let volume: Vec<f64> = bars.iter().map(|bar| bar.volume).collect();
    let capital: Vec<f64> = bars.iter().map(|bar| bar.capital).collect();

    let ema10 = ema(&close, 10);
    let ema20 = ema(&close, 20);
    let swl: Vec<f64> = ema10
        .iter()
        .zip(&ema20)
        .map(|(left, right)| (*left * 7.0 + *right * 3.0) / 10.0)
        .collect();
    let vol_sum_5 = rolling_sum(&volume, 5);
    let alpha: Vec<f64> = vol_sum_5
        .iter()
        .zip(&capital)
        .map(|(vol_sum, capital)| {
            ((100.0 * *vol_sum / (3.0 * *capital)).max(1.0) / 100.0).clamp(0.01, 1.0)
        })
        .collect();
    let sws = dma(&ema20, &alpha);

    let var1 = primary_state(&close, true);
    let vard = primary_state(&close, false);
    let red_states = alternating_states(&var1, &close, true);
    let cyan_states = alternating_states(&vard, &close, false);
    let red_hold = or_states(&red_states);
    let cyan_watch = or_states(&cyan_states);

    let ma34 = ma(&close, 34);
    let ma3 = ma(&close, 3);
    let bull_line = ma(&close, 26);
    let star_line = star_line(&close, &open, &high, &low);
    let (k, d, j) = kdj(&close, &high, &low);
    let quant_score = quant_scores(&close, &high, &low, &volume);
    let capital_behavior = capital_behavior_metrics(&close, &open, &high, &low, &volume);

    let mut computed = Vec::with_capacity(bars.len());
    for index in 0..bars.len() {
        let e_value = (high[index] + low[index] + open[index] + 2.0 * close[index]) / 5.0;
        let short_buy = index > 0 && cyan_watch[index - 1] && var1[index];
        let white_exit = index > 0 && red_hold[index - 1] && vard[index];
        let oversold =
            ma34[index].is_finite() && ((close[index] - ma34[index]) / ma34[index] * 100.0) < -14.0;
        let wait_line = if ma3[index].is_finite()
            && star_line[index].is_finite()
            && ma3[index] > star_line[index]
        {
            star_line[index]
        } else {
            ma3[index]
        };
        let previous_close = if index > 0 && close[index - 1].is_finite() {
            Some(close[index - 1])
        } else {
            None
        };
        let close_change = previous_close.map(|previous| close[index] - previous);
        let close_change_pct = previous_close
            .filter(|previous| previous.abs() > f64::EPSILON)
            .map(|previous| (close[index] - previous) / previous);
        computed.push(ComputedTrendBar {
            date: bars[index].date,
            close: close[index],
            open: open[index],
            high: high[index],
            low: low[index],
            volume: volume[index],
            previous_close,
            close_change,
            close_change_pct,
            swl: swl[index],
            sws: sws[index],
            k: k[index],
            d: d[index],
            j: j[index],
            accumulation_index: capital_behavior[index].accumulation_index,
            accumulation_strength: capital_behavior[index].accumulation_strength,
            swing_opportunity: capital_behavior[index].swing_opportunity,
            rebound_signal: capital_behavior[index].rebound_signal,
            trend_heat: capital_behavior[index].trend_heat,
            volume_price_heat: capital_behavior[index].volume_price_heat,
            anomaly_heat: capital_behavior[index].anomaly_heat,
            popularity_heat: capital_behavior[index].popularity_heat,
            star_line: star_line[index],
            bull_line: bull_line[index],
            wait_line,
            support: 2.0 * e_value - high[index],
            resistance: 2.0 * e_value - low[index],
            breakout: e_value + (high[index] - low[index]),
            reversal: e_value - (high[index] - low[index]),
            swl_above_sws: swl[index] > sws[index],
            kdj_golden_cross: index > 0 && k[index - 1] <= d[index - 1] && k[index] > d[index],
            kdj_dead_cross: index > 0 && k[index - 1] >= d[index - 1] && k[index] < d[index],
            kdj_overbought: k[index] >= 80.0 || d[index] >= 80.0 || j[index] >= 100.0,
            kdj_oversold: k[index] <= 20.0 || d[index] <= 20.0 || j[index] <= 0.0,
            red_hold: red_hold[index],
            cyan_watch: cyan_watch[index],
            short_buy,
            white_exit,
            oversold,
            quant_score: quant_score[index],
        });
    }
    computed
}

pub fn primary_state(close: &[f64], up: bool) -> Vec<bool> {
    let mut state = vec![false; close.len()];
    for index in 2..close.len() {
        state[index] = if up {
            close[index] > close[index - 1] && close[index] > close[index - 2]
        } else {
            close[index] < close[index - 1] && close[index] < close[index - 2]
        };
    }
    state
}

pub fn alternating_states(
    primary: &[bool],
    close: &[f64],
    starts_with_le_ge: bool,
) -> Vec<Vec<bool>> {
    let mut states = vec![primary.to_vec()];
    for state_index in 1..12 {
        let previous = states.last().expect("state exists");
        let le_ge = if state_index % 2 == 1 {
            starts_with_le_ge
        } else {
            !starts_with_le_ge
        };
        let mut next = vec![false; close.len()];
        for index in 2..close.len() {
            let condition = if le_ge {
                close[index] <= close[index - 1] && close[index] >= close[index - 2]
            } else {
                close[index] >= close[index - 1] && close[index] <= close[index - 2]
            };
            next[index] = previous[index - 1] && condition;
        }
        states.push(next);
    }
    states
}

pub fn or_states(states: &[Vec<bool>]) -> Vec<bool> {
    if states.is_empty() {
        return Vec::new();
    }
    let mut combined = vec![false; states[0].len()];
    for state in states {
        for (index, value) in state.iter().enumerate() {
            combined[index] |= *value;
        }
    }
    combined
}

pub fn ema(values: &[f64], span: usize) -> Vec<f64> {
    let alpha = 2.0 / (span as f64 + 1.0);
    let mut output = Vec::with_capacity(values.len());
    let mut previous = None;
    for value in values {
        let current = match previous {
            Some(previous_value) => alpha * *value + (1.0 - alpha) * previous_value,
            None => *value,
        };
        previous = Some(current);
        output.push(current);
    }
    output
}

pub fn ma(values: &[f64], window: usize) -> Vec<f64> {
    let mut output = vec![f64::NAN; values.len()];
    let mut sum = 0.0;
    for index in 0..values.len() {
        sum += values[index];
        if index >= window {
            sum -= values[index - window];
        }
        if index + 1 >= window {
            output[index] = sum / window as f64;
        }
    }
    output
}

pub fn rolling_sum(values: &[f64], window: usize) -> Vec<f64> {
    let mut output = Vec::with_capacity(values.len());
    let mut sum = 0.0;
    for index in 0..values.len() {
        sum += values[index];
        if index >= window {
            sum -= values[index - window];
        }
        output.push(sum);
    }
    output
}

pub fn rolling_mean_partial(values: &[f64], window: usize) -> Vec<f64> {
    let mut output = Vec::with_capacity(values.len());
    let mut sum = 0.0;
    for index in 0..values.len() {
        sum += values[index];
        if index >= window {
            sum -= values[index - window];
        }
        let count = (index + 1).min(window).max(1);
        output.push(sum / count as f64);
    }
    output
}

pub fn rolling_min(values: &[f64], window: usize) -> Vec<f64> {
    (0..values.len())
        .map(|index| {
            let start = index.saturating_sub(window.saturating_sub(1));
            values[start..=index]
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min)
        })
        .collect()
}

pub fn rolling_max(values: &[f64], window: usize) -> Vec<f64> {
    (0..values.len())
        .map(|index| {
            let start = index.saturating_sub(window.saturating_sub(1));
            values[start..=index]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect()
}

pub fn dma(values: &[f64], alpha: &[f64]) -> Vec<f64> {
    let mut output = Vec::with_capacity(values.len());
    let mut previous = None;
    for (value, coefficient) in values.iter().zip(alpha) {
        let coefficient = (*coefficient).clamp(0.0, 1.0);
        let current = match previous {
            Some(previous_value) => coefficient * *value + (1.0 - coefficient) * previous_value,
            None => *value,
        };
        previous = Some(current);
        output.push(current);
    }
    output
}

pub fn tdx_sma(values: &[f64], window: f64, weight: f64) -> Vec<f64> {
    let mut output = Vec::with_capacity(values.len());
    let mut previous = None;
    for value in values {
        let value = if value.is_finite() { *value } else { 50.0 };
        let current = match previous {
            Some(previous_value) => (weight * value + (window - weight) * previous_value) / window,
            None => value,
        };
        previous = Some(current);
        output.push(current);
    }
    output
}

pub fn kdj(close: &[f64], high: &[f64], low: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let low_min = rolling_min(low, 9);
    let high_max = rolling_max(high, 9);
    let rsv: Vec<f64> = close
        .iter()
        .enumerate()
        .map(|(index, close)| {
            let spread = high_max[index] - low_min[index];
            if spread == 0.0 {
                f64::NAN
            } else {
                ((*close - low_min[index]) / spread * 100.0).clamp(0.0, 100.0)
            }
        })
        .collect();
    let k = tdx_sma(&rsv, 3.0, 1.0);
    let d = tdx_sma(&k, 3.0, 1.0);
    let j = k.iter().zip(&d).map(|(k, d)| 3.0 * k - 2.0 * d).collect();
    (k, d, j)
}

pub fn quant_scores(close: &[f64], high: &[f64], low: &[f64], volume: &[f64]) -> Vec<i32> {
    let ma5 = ma(close, 5);
    let ma10 = ma(close, 10);
    let ma20 = ma(close, 20);
    let ma60 = ma(close, 60);
    let volume_ma60 = ma(volume, 60);
    let (k, _d, j) = kdj(close, high, low);
    let ema12 = ema(close, 12);
    let ema26 = ema(close, 26);
    let dif: Vec<f64> = ema12
        .iter()
        .zip(ema26)
        .map(|(left, right)| *left - right)
        .collect();
    let dea = ema(&dif, 9);
    let macd: Vec<f64> = dif
        .iter()
        .zip(&dea)
        .map(|(dif, dea)| 2.0 * (*dif - *dea))
        .collect();

    let mut scores = vec![0; close.len()];
    for index in 0..close.len() {
        if ma5[index] > ma10[index] {
            scores[index] += 20;
        }
        if ma20[index] > ma60[index] {
            scores[index] += 10;
        }
        if j[index] > k[index] {
            scores[index] += 10;
        }
        if dif[index] > dea[index] {
            scores[index] += 10;
        }
        if macd[index] > 0.0 {
            scores[index] += 10;
        }
        if volume[index] > volume_ma60[index] {
            scores[index] += 10;
        }
        if index > 0 && close[index] / close[index - 1] > 1.03 {
            scores[index] += 10;
        }
    }
    scores
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CapitalBehaviorBar {
    pub(crate) accumulation_index: f64,
    pub(crate) accumulation_strength: f64,
    pub(crate) swing_opportunity: f64,
    pub(crate) rebound_signal: f64,
    pub(crate) trend_heat: f64,
    pub(crate) volume_price_heat: f64,
    pub(crate) anomaly_heat: f64,
    pub(crate) popularity_heat: f64,
}

pub fn capital_behavior_metrics(
    close: &[f64],
    open: &[f64],
    high: &[f64],
    low: &[f64],
    volume: &[f64],
) -> Vec<CapitalBehaviorBar> {
    let len = close.len();
    if len == 0 {
        return Vec::new();
    }

    let volume_ma20 = rolling_mean_partial(volume, 20);
    let low_60 = rolling_min(low, 60);
    let high_60 = rolling_max(high, 60);
    let ma5 = ma(close, 5);
    let ma10 = ma(close, 10);
    let ma20 = ma(close, 20);
    let ma60 = ma(close, 60);

    let mut raw = Vec::with_capacity(len);
    let mut low_zone_values = Vec::with_capacity(len);
    let mut lower_shadow_values = Vec::with_capacity(len);
    let mut close_recovery_values = Vec::with_capacity(len);
    let mut volume_expansion_values = Vec::with_capacity(len);
    let mut price_position_values = Vec::with_capacity(len);
    let mut pct_change_values = Vec::with_capacity(len);
    let mut amplitude_values = Vec::with_capacity(len);
    let mut volume_ratio_values = Vec::with_capacity(len);

    for index in 0..len {
        let previous_close = if index > 0 && close[index - 1].abs() > f64::EPSILON {
            close[index - 1]
        } else {
            close[index].max(0.01)
        };
        let price_range = (high[index] - low[index]).max(0.0);
        let pct_change = close[index] / previous_close - 1.0;
        let amplitude = if previous_close.abs() > f64::EPSILON {
            price_range / previous_close
        } else {
            0.0
        };
        let volume_ratio = if volume_ma20[index].abs() > f64::EPSILON {
            (volume[index] / volume_ma20[index]).clamp(0.0, 4.0)
        } else {
            1.0
        };
        let rolling_range = high_60[index] - low_60[index];
        let price_position = if rolling_range.abs() > f64::EPSILON {
            ((close[index] - low_60[index]) / rolling_range).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let low_zone = 1.0 - price_position;
        let lower_shadow = if price_range.abs() > f64::EPSILON {
            ((open[index].min(close[index]) - low[index]) / price_range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let close_recovery = if price_range.abs() > f64::EPSILON {
            ((close[index] - low[index]) / price_range).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let volume_expansion = ((volume_ratio - 1.0).clamp(0.0, 2.0) / 2.0).clamp(0.0, 1.0);
        let positive_close = if close[index] >= open[index] {
            1.0
        } else {
            0.0
        };
        let negative_close = if close[index] < open[index] { 1.0 } else { 0.0 };

        raw.push(
            low_zone * 44.0
                + volume_expansion * 28.0
                + lower_shadow * 18.0
                + close_recovery * 8.0
                + positive_close * 4.0
                - price_position * 22.0
                - negative_close * pct_change.abs().min(0.08) * 260.0
                - if volume_ratio > 1.8 && close[index] < open[index] {
                    12.0
                } else {
                    0.0
                },
        );
        low_zone_values.push(low_zone);
        lower_shadow_values.push(lower_shadow);
        close_recovery_values.push(close_recovery);
        volume_expansion_values.push(volume_expansion);
        price_position_values.push(price_position);
        pct_change_values.push(pct_change);
        amplitude_values.push(amplitude);
        volume_ratio_values.push(volume_ratio);
    }

    let accumulation_index: Vec<f64> = ema(&raw, 5)
        .into_iter()
        .map(|value| (value - 34.0).clamp(-100.0, 100.0))
        .collect();

    let mut strength_raw = Vec::with_capacity(len);
    let mut swing_raw = Vec::with_capacity(len);
    let mut rebound_raw = Vec::with_capacity(len);
    let mut trend_heat = Vec::with_capacity(len);
    let mut volume_price_heat = Vec::with_capacity(len);
    let mut anomaly_heat = Vec::with_capacity(len);
    let mut popularity_heat = Vec::with_capacity(len);

    for index in 0..len {
        let low_zone = low_zone_values[index];
        let lower_shadow = lower_shadow_values[index];
        let close_recovery = close_recovery_values[index];
        let volume_expansion = volume_expansion_values[index];
        let price_position = price_position_values[index];
        let pct_change = pct_change_values[index];
        let amplitude = amplitude_values[index];
        let volume_ratio = volume_ratio_values[index];

        strength_raw.push(
            (accumulation_index[index].max(0.0) * 0.72
                + low_zone * 18.0
                + lower_shadow * 12.0
                + volume_expansion * 18.0)
                .clamp(0.0, 100.0),
        );

        let trend_turn = if ma5[index] > ma10[index] { 22.0 } else { 0.0 }
            + if ma10[index] > ma20[index] { 18.0 } else { 0.0 }
            + if close[index] > ma20[index] {
                18.0
            } else {
                0.0
            }
            + if index >= 5 && (ma20[index] - ma20[index - 5]) > 0.0 {
                14.0
            } else {
                0.0
            };
        swing_raw.push(
            (strength_raw[index] * 0.48 + trend_turn * 0.46 + low_zone * 16.0
                - price_position * 18.0)
                .clamp(0.0, 100.0),
        );
        rebound_raw.push(
            (low_zone * 32.0
                + lower_shadow * 28.0
                + volume_expansion * 20.0
                + if pct_change > 0.0 { 12.0 } else { 0.0 }
                + if index > 0 && pct_change_values[index - 1] < -0.025 {
                    8.0
                } else {
                    0.0
                })
            .clamp(0.0, 100.0),
        );

        let trend_value = if close[index] > ma5[index] { 20.0 } else { 0.0 }
            + if ma5[index] > ma10[index] { 25.0 } else { 0.0 }
            + if ma10[index] > ma20[index] { 25.0 } else { 0.0 }
            + if ma20[index] > ma60[index] { 15.0 } else { 0.0 }
            + if index >= 5 && (ma20[index] - ma20[index - 5]) > 0.0 {
                15.0
            } else {
                0.0
            };
        let volume_price_value = (volume_expansion * 42.0
            + pct_change.clamp(0.0, 0.06) * 520.0
            + close_recovery * 18.0
            + if close[index] > open[index] { 8.0 } else { 0.0 })
        .clamp(0.0, 100.0);
        let anomaly_value = (amplitude.min(0.12) * 360.0
            + (volume_ratio - 1.0).clamp(0.0, 3.0) * 20.0
            + pct_change.abs().min(0.08) * 260.0)
            .clamp(0.0, 100.0);
        let popularity_value = ((volume_ratio.min(3.0) / 3.0) * 36.0
            + amplitude.min(0.1) * 260.0
            + pct_change.clamp(0.0, 0.06) * 360.0
            + trend_value * 0.18)
            .clamp(0.0, 100.0);
        trend_heat.push(trend_value.clamp(0.0, 100.0));
        volume_price_heat.push(volume_price_value);
        anomaly_heat.push(anomaly_value);
        popularity_heat.push(popularity_value);
    }

    let accumulation_strength = ema(&strength_raw, 3);
    let swing_opportunity = ema(&swing_raw, 3);
    let rebound_signal = ema(&rebound_raw, 3);

    (0..len)
        .map(|index| CapitalBehaviorBar {
            accumulation_index: accumulation_index[index],
            accumulation_strength: accumulation_strength[index].clamp(0.0, 100.0),
            swing_opportunity: swing_opportunity[index].clamp(0.0, 100.0),
            rebound_signal: rebound_signal[index].clamp(0.0, 100.0),
            trend_heat: trend_heat[index],
            volume_price_heat: volume_price_heat[index],
            anomaly_heat: anomaly_heat[index],
            popularity_heat: popularity_heat[index],
        })
        .collect()
}

pub fn star_line(close: &[f64], open: &[f64], high: &[f64], low: &[f64]) -> Vec<f64> {
    let ytsl: Vec<f64> = close
        .iter()
        .enumerate()
        .map(|(index, close)| (3.0 * close + low[index] + open[index] + high[index]) / 6.0)
        .collect();
    let mut output = vec![f64::NAN; close.len()];
    for index in 20..close.len() {
        let mut total = 0.0;
        for (offset, weight) in (0..19).zip((2..=20).rev()) {
            total += ytsl[index - offset] * weight as f64;
        }
        total += ytsl[index - 20];
        output[index] = total / 211.0;
    }
    output
}

pub fn trend_signal_from_bar(code: &str, bar: &ComputedTrendBar) -> TrendIndicatorSignal {
    let reasons = trend_reasons(bar);
    TrendIndicatorSignal {
        code: code.to_string(),
        date: bar.date.format("%Y-%m-%d").to_string(),
        close: round4(bar.close),
        previous_close: bar.previous_close.and_then(finite_round4),
        close_change: bar.close_change.and_then(finite_round4),
        close_change_pct: bar.close_change_pct.and_then(finite_round4),
        swl: finite_round4(bar.swl),
        sws: finite_round4(bar.sws),
        k: finite_round4(bar.k),
        d: finite_round4(bar.d),
        j: finite_round4(bar.j),
        star_line: finite_round4(bar.star_line),
        bull_line: finite_round4(bar.bull_line),
        wait_line: finite_round4(bar.wait_line),
        support: finite_round4(bar.support),
        resistance: finite_round4(bar.resistance),
        breakout: finite_round4(bar.breakout),
        reversal: finite_round4(bar.reversal),
        swl_above_sws: bar.swl_above_sws,
        kdj_golden_cross: bar.kdj_golden_cross,
        kdj_dead_cross: bar.kdj_dead_cross,
        kdj_overbought: bar.kdj_overbought,
        kdj_oversold: bar.kdj_oversold,
        red_hold: bar.red_hold,
        cyan_watch: bar.cyan_watch,
        short_buy: bar.short_buy,
        white_exit: bar.white_exit,
        oversold: bar.oversold,
        signal_type: default_trend_signal_type(),
        risk_flags: trend_risk_flags_from_bar(bar),
        technical_score: None,
        pattern_layer_score: None,
        quality_score: None,
        quant_score: bar.quant_score,
        quant_score_max: default_quant_score_max(),
        pattern_score: pattern_score(bar),
        pattern_score_max: default_pattern_score_max(),
        pattern_signals: pattern_signals(bar),
        status: trend_status(bar),
        reasons,
        notes: trend_notes(),
    }
}

pub fn trend_point_from_bar(bar: &ComputedTrendBar) -> TrendIndicatorPoint {
    TrendIndicatorPoint {
        date: bar.date.format("%Y-%m-%d").to_string(),
        close: round4(bar.close),
        open: finite_round4(bar.open),
        high: finite_round4(bar.high),
        low: finite_round4(bar.low),
        volume: finite_round4(bar.volume),
        swl: finite_round4(bar.swl),
        sws: finite_round4(bar.sws),
        k: finite_round4(bar.k),
        d: finite_round4(bar.d),
        j: finite_round4(bar.j),
        accumulation_index: finite_round4(bar.accumulation_index),
        accumulation_strength: finite_round4(bar.accumulation_strength),
        swing_opportunity: finite_round4(bar.swing_opportunity),
        rebound_signal: finite_round4(bar.rebound_signal),
        trend_heat: finite_round4(bar.trend_heat),
        volume_price_heat: finite_round4(bar.volume_price_heat),
        anomaly_heat: finite_round4(bar.anomaly_heat),
        popularity_heat: finite_round4(bar.popularity_heat),
        red_hold: bar.red_hold,
        cyan_watch: bar.cyan_watch,
        short_buy: bar.short_buy,
        white_exit: bar.white_exit,
    }
}

pub fn trend_reasons(bar: &ComputedTrendBar) -> Vec<String> {
    let mut reasons = Vec::new();
    if bar.short_buy {
        reasons.push("short_buy_signal".to_string());
    }
    if bar.red_hold {
        reasons.push("red_hold".to_string());
    }
    if bar.swl_above_sws {
        reasons.push("swl_above_sws".to_string());
    }
    if bar.kdj_golden_cross {
        reasons.push("kdj_golden_cross".to_string());
    }
    if bar.kdj_dead_cross {
        reasons.push("kdj_dead_cross".to_string());
    }
    if bar.kdj_oversold {
        reasons.push("kdj_oversold".to_string());
    }
    if bar.kdj_overbought {
        reasons.push("kdj_overbought".to_string());
    }
    if bar.quant_score >= 60 {
        reasons.push("high_quant_score".to_string());
    }
    if bar.white_exit {
        reasons.push("white_exit".to_string());
    }
    if bar.cyan_watch {
        reasons.push("cyan_watch".to_string());
    }
    if bar.oversold {
        reasons.push("oversold".to_string());
    }
    if bar.accumulation_strength >= 55.0 {
        reasons.push("accumulation_strength".to_string());
    }
    if bar.swing_opportunity >= 60.0 {
        reasons.push("swing_opportunity".to_string());
    }
    reasons
}

pub fn pattern_score(bar: &ComputedTrendBar) -> i32 {
    let values = [
        bar.accumulation_strength,
        bar.swing_opportunity,
        bar.trend_heat,
        bar.volume_price_heat,
        bar.anomaly_heat,
        bar.popularity_heat,
    ];
    (values.iter().sum::<f64>() / values.len().max(1) as f64).round() as i32
}

pub fn pattern_signals(bar: &ComputedTrendBar) -> Vec<String> {
    let mut signals = Vec::new();
    if bar.accumulation_index > 8.0 && bar.accumulation_strength >= 45.0 {
        signals.push("bottom_accumulation".to_string());
    }
    if bar.swing_opportunity >= 60.0 {
        signals.push("swing_opportunity".to_string());
    }
    if bar.rebound_signal >= 62.0 {
        signals.push("rebound_signal".to_string());
    }
    if bar.trend_heat >= 65.0 && bar.volume_price_heat >= 55.0 {
        signals.push("dragon_trend_volume".to_string());
    }
    signals
}

pub fn trend_status(bar: &ComputedTrendBar) -> String {
    if bar.short_buy {
        "buy_setup"
    } else if bar.white_exit {
        "exit"
    } else if bar.red_hold && bar.swl_above_sws {
        "uptrend"
    } else if bar.red_hold {
        "hold"
    } else if bar.cyan_watch {
        "watch"
    } else if bar.oversold {
        "oversold"
    } else {
        "neutral"
    }
    .to_string()
}

#[derive(Clone, Debug)]
pub struct TrendLayerScores {
    pub(crate) technical_score: f64,
    pub(crate) pattern_score: f64,
    pub(crate) quality_score: f64,
    pub(crate) trend_score: f64,
    pub(crate) final_score: f64,
    pub(crate) signal_type: String,
    pub(crate) risk_flags: Vec<String>,
    pub(crate) reason_tags: Vec<String>,
}

pub fn layered_trend_score(stock: &StockItem, trend: &TrendIndicatorResult) -> TrendLayerScores {
    let signal = &trend.signal;
    let bars = &trend.series;
    let technical_score = technical_layer_score(signal, bars);
    let pattern_score = pattern_layer_score(signal, bars);
    let quality_score = quality_layer_score(stock);
    let mut risk_flags = trend_layer_risk_flags(signal, bars);
    let mut final_score = technical_score * 0.70 + pattern_score * 0.20 + quality_score * 0.10;
    if risk_flags
        .iter()
        .any(|flag| flag == "breakdown_ma20" || flag == "bearish_long_ma_stack")
    {
        final_score -= 18.0;
    }
    if risk_flags
        .iter()
        .any(|flag| flag == "macd_bearish_divergence" || flag == "volume_stall")
    {
        final_score -= 8.0;
    }
    final_score = final_score.clamp(0.0, 100.0);
    let signal_type = classify_trend_signal(signal, bars, final_score, &risk_flags);
    if signal_type == "risk_warning" && !risk_flags.iter().any(|flag| flag == "weak_final_score") {
        risk_flags.push("weak_final_score".to_string());
    }
    risk_flags.sort();
    risk_flags.dedup();
    let reason_tags = layered_reason_tags(
        signal,
        bars,
        &signal_type,
        technical_score,
        pattern_score,
        quality_score,
    );
    TrendLayerScores {
        technical_score: round2(technical_score),
        pattern_score: round2(pattern_score),
        quality_score: round2(quality_score),
        trend_score: round2(final_score),
        final_score: round2(final_score),
        signal_type,
        risk_flags,
        reason_tags,
    }
}

pub fn trend_screen_candidate_allowed(layered: &TrendLayerScores) -> bool {
    !layered
        .risk_flags
        .iter()
        .any(|flag| flag == "bearish_long_ma_stack")
}

pub fn technical_layer_score(signal: &TrendIndicatorSignal, bars: &[TrendIndicatorPoint]) -> f64 {
    if bars.len() < 45 {
        return 0.0;
    }
    let closes = closes_from_points(bars);
    let volumes = volumes_from_points(bars);
    let Some(close) = closes
        .last()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
    else {
        return 0.0;
    };
    let ma5 = trailing_mean(&closes, 5);
    let ma10 = trailing_mean(&closes, 10);
    let ma20 = trailing_mean(&closes, 20);
    let ma60 = trailing_mean(&closes, 60);
    let high20 = trailing_max(&closes, 20);
    let high60 = trailing_max(&closes, 60);
    let low60 = trailing_min(&closes, 60);
    let volume5 = trailing_mean(&volumes, 5);
    let volume20 = trailing_mean(&volumes, 20);
    let ret20 = closes
        .len()
        .checked_sub(21)
        .and_then(|idx| pct_change(close, closes[idx]))
        .unwrap_or(0.0);
    let ret60 = closes
        .len()
        .checked_sub(61)
        .and_then(|idx| pct_change(close, closes[idx]))
        .unwrap_or(ret20);
    let range_position60 = if high60 > low60 {
        ((close - low60) / (high60 - low60)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let drawdown20 = if high20 > 0.0 {
        (close / high20 - 1.0).min(0.0).abs() * 100.0
    } else {
        100.0
    };
    let macd = macd_snapshot(&closes);
    let mut score = 0.0;

    if ma5 > ma10 && ma10 > ma20 && ma20 > ma60 {
        score += 18.0;
    } else if ma5 > ma10 && ma10 > ma20 {
        score += 12.0;
    } else if close > ma20 && ma20 > ma60 {
        score += 8.0;
    }
    if close > ma5 {
        score += 4.0;
    }
    if close > ma20 {
        score += 5.0;
    }
    if close > ma60 {
        score += 5.0;
    }
    if ma20 > ma60 {
        score += 5.0;
    }
    score += (ret20.clamp(-8.0, 18.0) + 8.0) / 26.0 * 10.0;
    score += (ret60.clamp(-12.0, 45.0) + 12.0) / 57.0 * 8.0;
    score += range_position60 * 7.0;
    score += (1.0 - (drawdown20 / 18.0).clamp(0.0, 1.0)) * 7.0;

    if macd.dif > macd.dea && macd.hist > 0.0 {
        score += 8.0;
    }
    if macd.prev_dif <= macd.prev_dea && macd.dif > macd.dea {
        score += 7.0;
    }
    if macd.hist > macd.prev_hist {
        score += 5.0;
    }
    if signal.kdj_golden_cross {
        score += 7.0;
    }
    if signal.kdj_oversold && signal.k.unwrap_or(0.0) > signal.d.unwrap_or(0.0) {
        score += 4.0;
    }
    if volume20 > 0.0 && volume5 > volume20 * 1.15 && close >= high20 * 0.98 {
        score += 7.0;
    }
    if volume20 > 0.0 && volume5 <= volume20 * 0.95 && close >= ma20 * 0.98 && close <= ma20 * 1.08
    {
        score += 5.0;
    }
    if signal.swl_above_sws {
        score += 7.0;
    }
    if signal
        .swl
        .zip(signal.sws)
        .map(|(swl, sws)| swl > sws * 1.01)
        .unwrap_or(false)
    {
        score += 3.0;
    }
    if signal.quant_score_max > 0 {
        score += (signal.quant_score as f64 / signal.quant_score_max as f64).clamp(0.0, 1.0) * 8.0;
    }
    score.clamp(0.0, 100.0)
}

pub fn pattern_layer_score(signal: &TrendIndicatorSignal, bars: &[TrendIndicatorPoint]) -> f64 {
    let closes = closes_from_points(bars);
    let volumes = volumes_from_points(bars);
    let close = closes.last().copied().unwrap_or(signal.close);
    let ma20 = trailing_mean(&closes, 20);
    let ma60 = trailing_mean(&closes, 60);
    let high20 = trailing_max(&closes, 20);
    let volume5 = trailing_mean(&volumes, 5);
    let volume20 = trailing_mean(&volumes, 20);
    let mut score = (signal.pattern_score as f64 / signal.pattern_score_max.max(1) as f64 * 45.0)
        .clamp(0.0, 45.0);
    if close >= high20 * 0.985 && volume20 > 0.0 && volume5 > volume20 * 1.1 {
        score += 18.0;
    }
    if close >= ma20 * 0.98 && close <= ma20 * 1.06 && volume20 > 0.0 && volume5 <= volume20 * 1.05
    {
        score += 14.0;
    }
    if close >= ma60 * 0.98 && close <= ma60 * 1.08 {
        score += 8.0;
    }
    if signal
        .support
        .map(|support| close >= support * 0.98 && close <= support * 1.08)
        .unwrap_or(false)
    {
        score += 7.0;
    }
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "dragon_trend_volume")
    {
        score += 8.0;
    }
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "swing_opportunity")
    {
        score += 6.0;
    }
    if signal.white_exit {
        score -= 16.0;
    }
    if signal.kdj_overbought {
        score -= 8.0;
    }
    if high_upper_shadow_ratio(bars).unwrap_or(0.0) >= 0.45 {
        score -= 8.0;
    }
    score.clamp(0.0, 100.0)
}

pub fn quality_layer_score(stock: &StockItem) -> f64 {
    let mut score: f64 = 45.0;
    if let Some(roe) = stock.roe.and_then(ratio_percentage_points) {
        score += if roe >= 15.0 {
            18.0
        } else if roe >= 8.0 {
            10.0
        } else if roe >= 0.0 {
            2.0
        } else {
            -14.0
        };
    }
    if stock
        .deducted_net_profit_billion
        .map(|value| value > 0.0)
        .unwrap_or(false)
    {
        score += 14.0;
    } else {
        score -= 8.0;
    }
    if let Some(growth) = stock
        .deducted_net_profit_growth_rate
        .and_then(percentage_points)
    {
        score += if growth >= 20.0 {
            10.0
        } else if growth >= 10.0 {
            6.0
        } else if growth >= 0.0 {
            2.0
        } else {
            -10.0
        };
    }
    if let Some(margin) = stock.deducted_net_profit_margin.and_then(percentage_points) {
        score += if margin >= 15.0 {
            6.0
        } else if margin >= 8.0 {
            3.0
        } else {
            0.0
        };
    }
    if let Some(pe) = stock.pe.filter(|value| value.is_finite() && *value > 0.0) {
        score += if pe <= 35.0 {
            4.0
        } else if pe <= 60.0 {
            0.0
        } else {
            -6.0
        };
    }
    if let Some(pb) = stock.pb.filter(|value| value.is_finite() && *value > 0.0) {
        score += if pb <= 6.0 { 4.0 } else { -4.0 };
    }
    score.clamp(0.0, 100.0)
}

pub fn classify_trend_signal(
    signal: &TrendIndicatorSignal,
    bars: &[TrendIndicatorPoint],
    final_score: f64,
    risk_flags: &[String],
) -> String {
    if final_score < 65.0
        || risk_flags
            .iter()
            .any(|flag| flag == "breakdown_ma20" || flag == "bearish_long_ma_stack")
    {
        return "risk_warning".to_string();
    }
    let closes = closes_from_points(bars);
    let volumes = volumes_from_points(bars);
    let close = closes.last().copied().unwrap_or(signal.close);
    let high20 = trailing_max(&closes, 20);
    let ma20 = trailing_mean(&closes, 20);
    let volume5 = trailing_mean(&volumes, 5);
    let volume20 = trailing_mean(&volumes, 20);
    if close >= high20 * 0.985 && volume20 > 0.0 && volume5 >= volume20 * 1.1 {
        return "breakout_chase".to_string();
    }
    if close >= ma20 * 0.98
        && close <= ma20 * 1.06
        && (signal.kdj_golden_cross || signal.swl_above_sws)
    {
        return "pullback_buy".to_string();
    }
    "trend_continuation".to_string()
}

pub fn layered_reason_tags(
    signal: &TrendIndicatorSignal,
    bars: &[TrendIndicatorPoint],
    signal_type: &str,
    technical: f64,
    pattern: f64,
    quality: f64,
) -> Vec<String> {
    let closes = closes_from_points(bars);
    let ma5 = trailing_mean(&closes, 5);
    let ma10 = trailing_mean(&closes, 10);
    let ma20 = trailing_mean(&closes, 20);
    let ma60 = trailing_mean(&closes, 60);
    let close = closes.last().copied().unwrap_or(signal.close);
    let mut tags = vec![format!("signal_type:{signal_type}")];
    if ma5 > ma10 && ma10 > ma20 && ma20 > ma60 {
        tags.push("ma_bull_stack".to_string());
    }
    if close > ma20 {
        tags.push("price_above_ma20".to_string());
    }
    if signal.swl_above_sws {
        tags.push("swl_strength".to_string());
    }
    if signal.kdj_golden_cross {
        tags.push("kdj_golden_cross".to_string());
    }
    if technical >= 75.0 {
        tags.push("technical_score_strong".to_string());
    }
    if pattern >= 70.0 {
        tags.push("pattern_score_strong".to_string());
    }
    if quality >= 70.0 {
        tags.push("quality_soft_bonus".to_string());
    }
    tags
}

pub fn trend_layer_risk_flags(
    signal: &TrendIndicatorSignal,
    bars: &[TrendIndicatorPoint],
) -> Vec<String> {
    let closes = closes_from_points(bars);
    let volumes = volumes_from_points(bars);
    let close = closes.last().copied().unwrap_or(signal.close);
    let ma20 = trailing_mean(&closes, 20);
    let ma60 = trailing_mean(&closes, 60);
    let ma120 = trailing_mean(&closes, 120);
    let volume5 = trailing_mean(&volumes, 5);
    let volume20 = trailing_mean(&volumes, 20);
    let macd = macd_snapshot(&closes);
    let mut flags = signal.risk_flags.clone();
    if bars.len() < 45 {
        flags.push("insufficient_history".to_string());
    }
    if volume20 <= 0.0 || !volume20.is_finite() {
        flags.push("low_volume".to_string());
    }
    if ma20.is_finite() && close < ma20 * 0.97 {
        flags.push("breakdown_ma20".to_string());
    }
    if ma120.is_finite() && ma20 < ma60 && ma60 < ma120 {
        flags.push("bearish_long_ma_stack".to_string());
    }
    if signal.kdj_overbought {
        flags.push("kdj_overbought".to_string());
    }
    if signal.white_exit {
        flags.push("white_exit".to_string());
    }
    if high_upper_shadow_ratio(bars).unwrap_or(0.0) >= 0.45
        && volume20 > 0.0
        && volume5 > volume20 * 1.1
    {
        flags.push("volume_stall".to_string());
    }
    if macd.dif > 0.0 && macd.hist < macd.prev_hist && close >= trailing_max(&closes, 20) * 0.98 {
        flags.push("macd_bearish_divergence".to_string());
    }
    flags.sort();
    flags.dedup();
    flags
}

pub fn trend_risk_flags_from_bar(bar: &ComputedTrendBar) -> Vec<String> {
    let mut flags = Vec::new();
    if bar.white_exit {
        flags.push("white_exit".to_string());
    }
    if bar.kdj_overbought {
        flags.push("kdj_overbought".to_string());
    }
    if bar.kdj_dead_cross {
        flags.push("kdj_dead_cross".to_string());
    }
    if !bar.swl_above_sws {
        flags.push("swl_below_sws".to_string());
    }
    flags
}

pub fn closes_from_points(bars: &[TrendIndicatorPoint]) -> Vec<f64> {
    bars.iter().map(|bar| bar.close).collect()
}

pub fn volumes_from_points(bars: &[TrendIndicatorPoint]) -> Vec<f64> {
    bars.iter()
        .map(|bar| bar.volume.unwrap_or(0.0).max(0.0))
        .collect()
}

pub fn trailing_mean(values: &[f64], window: usize) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let start = values.len().saturating_sub(window.max(1));
    let mut sum = 0.0;
    let mut count = 0.0;
    for value in values[start..]
        .iter()
        .copied()
        .filter(|value| value.is_finite())
    {
        sum += value;
        count += 1.0;
    }
    if count > 0.0 {
        sum / count
    } else {
        f64::NAN
    }
}

pub fn trailing_max(values: &[f64], window: usize) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let start = values.len().saturating_sub(window.max(1));
    values[start..]
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(f64::NEG_INFINITY, f64::max)
}

pub fn trailing_min(values: &[f64], window: usize) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let start = values.len().saturating_sub(window.max(1));
    values[start..]
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(f64::INFINITY, f64::min)
}

pub fn pct_change(current: f64, previous: f64) -> Option<f64> {
    if current.is_finite() && previous.is_finite() && previous.abs() > f64::EPSILON {
        Some((current / previous - 1.0) * 100.0)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MacdSnapshot {
    pub(crate) dif: f64,
    pub(crate) dea: f64,
    pub(crate) hist: f64,
    pub(crate) prev_dif: f64,
    pub(crate) prev_dea: f64,
    pub(crate) prev_hist: f64,
}

pub fn macd_snapshot(closes: &[f64]) -> MacdSnapshot {
    if closes.len() < 2 {
        return MacdSnapshot::default();
    }
    let ema12 = ema(closes, 12);
    let ema26 = ema(closes, 26);
    let dif = ema12
        .iter()
        .zip(ema26.iter())
        .map(|(left, right)| *left - *right)
        .collect::<Vec<_>>();
    let dea = ema(&dif, 9);
    let hist = dif
        .iter()
        .zip(dea.iter())
        .map(|(dif, dea)| 2.0 * (*dif - *dea))
        .collect::<Vec<_>>();
    let last = dif.len() - 1;
    let prev = last.saturating_sub(1);
    MacdSnapshot {
        dif: dif[last],
        dea: dea[last],
        hist: hist[last],
        prev_dif: dif[prev],
        prev_dea: dea[prev],
        prev_hist: hist[prev],
    }
}

pub fn high_upper_shadow_ratio(bars: &[TrendIndicatorPoint]) -> Option<f64> {
    let bar = bars.last()?;
    let high = bar.high?;
    let open = bar.open.unwrap_or(bar.close);
    let low = bar.low?;
    if !high.is_finite() || !open.is_finite() || !low.is_finite() || high <= low {
        return None;
    }
    Some(((high - open.max(bar.close)) / (high - low)).clamp(0.0, 1.0))
}
pub fn trend_screen_explanation(
    candidate: &ScreenedStock,
    signal: &TrendIndicatorSignal,
    layered: &TrendLayerScores,
) -> SelectionExplanation {
    let mut basis = vec![
        format!("通过基础筛选，基础评分 {:.2}。", candidate.score),
        format!(
            "{}：技术分 {:.2}、形态分 {:.2}、质量分 {:.2}。",
            trend_screen_signal_label(&signal.signal_type),
            layered.technical_score,
            layered.pattern_score,
            layered.quality_score
        ),
        format!(
            "日线截至 {}，收盘价 {:.2} 元；K={}、D={}、J={}。",
            signal.date,
            signal.close,
            trend_indicator_value(signal.k),
            trend_indicator_value(signal.d),
            trend_indicator_value(signal.j),
        ),
    ];
    if let Some(pattern_text) = pattern_basis_text(signal) {
        basis.push(pattern_text);
    }

    let mut score_breakdown = vec![
        score_contribution(
            "technical_score",
            "技术层",
            layered.technical_score,
            layered.technical_score * 0.70,
            75.0,
            60.0,
        ),
        score_contribution(
            "pattern_score",
            "形态层",
            layered.pattern_score,
            layered.pattern_score * 0.20,
            70.0,
            55.0,
        ),
        score_contribution(
            "quality_score",
            "质量层",
            layered.quality_score,
            layered.quality_score * 0.10,
            70.0,
            50.0,
        ),
    ];
    let adjustment = layered.final_score
        - score_breakdown
            .iter()
            .filter_map(|part| part.contribution)
            .sum::<f64>();
    if adjustment.abs() >= 0.01 {
        score_breakdown.push(ScoreContribution {
            key: "risk_adjustment".into(),
            label: "风险调整".into(),
            value: Some(round2(adjustment)),
            contribution: Some(round2(adjustment)),
            tone: "negative".into(),
        });
    }
    SelectionExplanation {
        basis,
        score_breakdown,
        risk_checks: trend_risk_checks(signal),
        verification: vec![
            "观察下一交易日的价格与成交量，确认信号是否延续。".to_string(),
            "结合突破、回踩、趋势延续和风险预警状态继续核验。".to_string(),
        ],
    }
}

pub fn trend_screen_signal_label(signal_type: &str) -> &str {
    match signal_type {
        "trend_continuation" => "趋势延续",
        "trend_reversal" => "趋势反转",
        "range_bound" => "区间震荡",
        "breakout_attempt" => "突破尝试",
        "breakout_chase" => "突破追踪",
        "pullback_buy" => "回踩观察",
        "risk_warning" => "风险预警",
        _ => "趋势观察",
    }
}

pub fn trend_indicator_value(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "—".into())
}

pub fn pattern_basis_text(signal: &TrendIndicatorSignal) -> Option<String> {
    let mut labels = Vec::new();
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "bottom_accumulation")
    {
        labels.push("底部吸筹");
    }
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "swing_opportunity")
    {
        labels.push("波段机会");
    }
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "rebound_signal")
    {
        labels.push("反弹信号");
    }
    if signal
        .pattern_signals
        .iter()
        .any(|value| value == "dragon_trend_volume")
    {
        labels.push("趋势量能共振");
    }
    if labels.is_empty() {
        None
    } else {
        Some(format!("形态信号：{}。", labels.join("、")))
    }
}

pub fn trend_risk_checks(signal: &TrendIndicatorSignal) -> Vec<String> {
    let mut risks = Vec::new();
    if signal.kdj_overbought {
        let zone = if signal.kdj_oversold {
            "KDJ高低位信号分歧"
        } else {
            "KDJ超买"
        };
        let triggers = [
            ("K", signal.k, 80.0),
            ("D", signal.d, 80.0),
            ("J", signal.j, 100.0),
        ]
        .into_iter()
        .filter_map(|(label, value, threshold)| {
            value
                .filter(|value| value.is_finite() && *value >= threshold)
                .map(|value| format!("{label}={value:.2}≥{threshold:.0}"))
        })
        .collect::<Vec<_>>();
        risks.push(format!(
            "{zone}：{} 日线，{}；需观察高位回落风险。",
            signal.date,
            triggers.join("、")
        ));
    }
    if signal.kdj_dead_cross {
        risks.push("KDJ死叉，短期动量转弱。".into());
    }
    if signal.white_exit {
        risks.push("白线离场信号已触发，需重新核验短线趋势。".into());
    }
    if signal.cyan_watch {
        risks.push("青线观察状态，趋势确认尚不充分。".into());
    }
    for flag in &signal.risk_flags {
        let text = match flag.as_str() {
            "swl_below_sws" => "短期均线低于长期均线。",
            "breakdown_ma20" => "跌破20日均线：收盘价低于均线3%以上。",
            "bearish_long_ma_stack" => "长期均线空头排列。",
            "volume_stall" => "放量滞涨，需观察上方抛压。",
            "macd_bearish_divergence" => "MACD动量减弱，存在顶背离风险。",
            "weak_final_score" => "趋势强度不足：综合技术评分或趋势结构未通过强势确认。",
            "insufficient_history" => "日线样本不足，技术评分参考性有限。",
            "low_volume" => "成交量不足，信号确认受限。",
            _ => continue,
        };
        risks.push(text.into());
    }
    if risks.is_empty() {
        risks.push("暂未触发已配置的技术风险规则，仍需后续价格与成交量确认。".into());
    }
    risks
}

pub fn trend_notes() -> Vec<String> {
    vec![
        "吸筹分析根据日线价量推断，暂未纳入真实筹码分布、龙虎榜席位和主力资金流。".to_string(),
        "量化评分满分为90分，筹码分布相关指标尚未纳入。".to_string(),
        "长期均线根据量能动态平滑，用于趋势参考。".to_string(),
    ]
}
