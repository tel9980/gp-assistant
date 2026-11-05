use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    sync::OnceLock,
};

use chrono::{Datelike, Days, Local, NaiveDate, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod adaptive_screen;
mod rates;
mod scoring_parameters;
mod source_fingerprint;
pub use scoring_parameters::{
    adaptive_score_factor_vector, fixed_scoring_parameters, scoring_implementation_fingerprint,
    validate_scoring_parameters, ModeParameters, ScoringParameters, METRIC_CONTRACT_VERSION,
    SCORING_REVISION,
};
pub use source_fingerprint::source_text_fingerprint;
mod industry_rank;
#[cfg(test)]
mod scoring_contract_tests;
use industry_rank::industry_rank_values;
use rates::{percentage_points, ratio_percentage_points};
pub use rates::{RateRatio, RateUnit};
mod volatility;
pub use adaptive_screen::{
    adaptive_candidate_codes, adaptive_scoring_features, adaptive_screen_stocks,
    adaptive_screen_stocks_with_parameters, AdaptiveFeatureRow, AdaptiveFeatureSet,
    AdaptiveMarketRegime, AdaptiveRecentExposure, AdaptiveScreenRequest, AdaptiveScreenResult,
    MIN_MARKET_BREADTH_COVERAGE,
};
pub use volatility::{
    AtrSnapshot, BollingerBandsSnapshot, ChaikinVolatilitySnapshot, DonchianChannelSnapshot,
    IndicatorUnavailable, KeltnerChannelSnapshot, RviSnapshot, VolatilitySnapshot,
};

#[derive(Debug, Clone)]
pub struct CoreError {
    message: String,
}

impl CoreError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CoreError {}

impl From<serde_json::Error> for CoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockItem {
    pub code: String,
    pub name: String,
    pub industry: String,
    #[serde(default)]
    pub is_st: bool,
    pub price: f64,
    #[serde(default)]
    pub pe: Option<f64>,
    #[serde(default)]
    pub pb: Option<f64>,
    #[serde(default)]
    pub roe: Option<f64>,
    #[serde(default)]
    pub market_cap_billion: Option<f64>,
    #[serde(default)]
    pub dividend_yield: Option<f64>,
    #[serde(default)]
    pub latest_eps: Option<f64>,
    #[serde(default)]
    pub deducted_net_profit_billion: Option<f64>,
    #[serde(default)]
    pub deducted_net_profit_margin: Option<f64>,
    #[serde(default)]
    pub deducted_net_profit_growth_rate: Option<f64>,
    #[serde(default)]
    pub change_pct: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub turnover_rate: Option<f64>,
    #[serde(default)]
    pub volume_ratio: Option<f64>,
    #[serde(default)]
    pub quote_time: Option<String>,
}

impl Default for StockItem {
    fn default() -> Self {
        Self {
            code: String::new(),
            name: String::new(),
            industry: String::new(),
            is_st: false,
            price: 0.0,
            pe: None,
            pb: None,
            roe: None,
            market_cap_billion: None,
            dividend_yield: None,
            latest_eps: None,
            deducted_net_profit_billion: None,
            deducted_net_profit_margin: None,
            deducted_net_profit_growth_rate: None,
            change_pct: None,
            volume: None,
            amount: None,
            turnover_rate: None,
            volume_ratio: None,
            quote_time: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenCriteria {
    #[serde(default)]
    pub min_roe: Option<f64>,
    #[serde(default)]
    pub max_pe: Option<f64>,
    #[serde(default)]
    pub max_pb: Option<f64>,
    #[serde(default)]
    pub min_market_cap_billion: Option<f64>,
    #[serde(default)]
    pub min_deducted_net_profit_billion: Option<f64>,
    #[serde(default)]
    pub min_deducted_net_profit_margin: Option<f64>,
    #[serde(default)]
    pub min_deducted_net_profit_growth_rate: Option<f64>,
    #[serde(default)]
    pub industry: Option<String>,
    #[serde(default)]
    pub market_scope: Option<String>,
    #[serde(default)]
    pub include_st: bool,
    #[serde(default = "default_screen_limit")]
    pub limit: usize,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_dir")]
    pub sort_dir: String,
    #[serde(default = "default_score_profile")]
    pub score_profile: String,
}

impl Default for ScreenCriteria {
    fn default() -> Self {
        Self {
            min_roe: None,
            max_pe: None,
            max_pb: None,
            min_market_cap_billion: None,
            min_deducted_net_profit_billion: None,
            min_deducted_net_profit_margin: None,
            min_deducted_net_profit_growth_rate: None,
            industry: None,
            market_scope: None,
            include_st: false,
            limit: default_screen_limit(),
            sort_by: default_sort_by(),
            sort_dir: default_sort_dir(),
            score_profile: default_score_profile(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenedStock {
    #[serde(default)]
    pub industry_sample_count: Option<usize>,
    pub stock: StockItem,
    pub score: f64,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub quality_score: f64,
    #[serde(default)]
    pub trend_score: f64,
    #[serde(default)]
    pub risk_score: f64,
    #[serde(default)]
    pub balanced_score: f64,
    #[serde(default)]
    pub factor_scores: BTreeMap<String, f64>,
    #[serde(default)]
    pub score_breakdown: Vec<ScoreContribution>,
    #[serde(default)]
    pub score_explanation: String,
    #[serde(default)]
    pub reason_tags: Vec<String>,
    #[serde(default)]
    pub risk_tags: Vec<String>,
    #[serde(default)]
    pub suitable_periods: Vec<String>,
    #[serde(default)]
    pub concept: Option<String>,
    #[serde(default)]
    pub theme_category: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenResultGroup {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub returned: usize,
    #[serde(default)]
    pub items: Vec<ScreenedStock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenResult {
    pub total: usize,
    pub returned: usize,
    pub items: Vec<ScreenedStock>,
    #[serde(default)]
    pub groups: Vec<ScreenResultGroup>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorScreenRequest {
    #[serde(default)]
    pub criteria: ScreenCriteria,
    #[serde(default = "default_sector_group_limit")]
    pub max_sectors: usize,
    #[serde(default = "default_per_sector_limit")]
    pub per_sector_limit: usize,
    #[serde(default = "default_min_sector_candidates")]
    pub min_sector_candidates: usize,
    #[serde(default = "default_sector_group_by")]
    pub group_by: String,
}

impl Default for SectorScreenRequest {
    fn default() -> Self {
        Self {
            criteria: ScreenCriteria::default(),
            max_sectors: default_sector_group_limit(),
            per_sector_limit: default_per_sector_limit(),
            min_sector_candidates: default_min_sector_candidates(),
            group_by: default_sector_group_by(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorScreenGroup {
    pub sector: String,
    pub total: usize,
    pub returned: usize,
    pub average_score: f64,
    pub items: Vec<ScreenedStock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorScreenResult {
    pub total: usize,
    pub returned: usize,
    pub sector_count: usize,
    pub groups: Vec<SectorScreenGroup>,
    #[serde(default)]
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockRelation {
    pub source_code: String,
    pub target_code: String,
    pub relation_type: String,
    #[serde(default = "default_relation_weight")]
    pub weight: f64,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoreContribution {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub contribution: Option<f64>,
    #[serde(default = "default_tone")]
    pub tone: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectionExplanation {
    #[serde(default)]
    pub basis: Vec<String>,
    #[serde(default)]
    pub score_breakdown: Vec<ScoreContribution>,
    #[serde(default)]
    pub risk_checks: Vec<String>,
    #[serde(default)]
    pub verification: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphCenterContext {
    #[serde(default = "default_graph_center_mode")]
    pub mode: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphScreenRequest {
    #[serde(default)]
    pub criteria: ScreenCriteria,
    #[serde(default)]
    pub seed_codes: Vec<String>,
    #[serde(default)]
    pub seed_query: String,
    #[serde(default = "default_relation_depth")]
    pub relation_depth: usize,
    #[serde(default = "default_relation_weight_param")]
    pub relation_weight: f64,
    #[serde(default = "default_graph_limit")]
    pub limit: usize,
}

impl Default for GraphScreenRequest {
    fn default() -> Self {
        Self {
            criteria: ScreenCriteria::default(),
            seed_codes: Vec::new(),
            seed_query: String::new(),
            relation_depth: default_relation_depth(),
            relation_weight: default_relation_weight_param(),
            limit: default_graph_limit(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphStockSignal {
    pub stock: StockItem,
    pub base_score: f64,
    pub relation_score: f64,
    pub final_score: f64,
    pub suggested_weight: f64,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub related: Vec<StockRelation>,
    #[serde(default)]
    pub explanation: SelectionExplanation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphScreenResult {
    pub total: usize,
    pub returned: usize,
    pub relation_count: usize,
    pub items: Vec<GraphStockSignal>,
    #[serde(default)]
    pub center_context: GraphCenterContext,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendIndicatorRequest {
    pub code: String,
    #[serde(default = "default_start_date")]
    pub start_date: String,
    #[serde(default = "default_end_date")]
    pub end_date: String,
    #[serde(default = "default_series_limit")]
    pub series_limit: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendIndicatorPoint {
    pub date: String,
    pub close: f64,
    #[serde(default)]
    pub open: Option<f64>,
    #[serde(default)]
    pub high: Option<f64>,
    #[serde(default)]
    pub low: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub swl: Option<f64>,
    #[serde(default)]
    pub sws: Option<f64>,
    #[serde(default)]
    pub k: Option<f64>,
    #[serde(default)]
    pub d: Option<f64>,
    #[serde(default)]
    pub j: Option<f64>,
    #[serde(default)]
    pub accumulation_index: Option<f64>,
    #[serde(default)]
    pub accumulation_strength: Option<f64>,
    #[serde(default)]
    pub swing_opportunity: Option<f64>,
    #[serde(default)]
    pub rebound_signal: Option<f64>,
    #[serde(default)]
    pub trend_heat: Option<f64>,
    #[serde(default)]
    pub volume_price_heat: Option<f64>,
    #[serde(default)]
    pub anomaly_heat: Option<f64>,
    #[serde(default)]
    pub popularity_heat: Option<f64>,
    #[serde(default)]
    pub red_hold: bool,
    #[serde(default)]
    pub cyan_watch: bool,
    #[serde(default)]
    pub short_buy: bool,
    #[serde(default)]
    pub white_exit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendIndicatorSignal {
    pub code: String,
    pub date: String,
    pub close: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_close: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_change: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_change_pct: Option<f64>,
    #[serde(default)]
    pub swl: Option<f64>,
    #[serde(default)]
    pub sws: Option<f64>,
    #[serde(default)]
    pub k: Option<f64>,
    #[serde(default)]
    pub d: Option<f64>,
    #[serde(default)]
    pub j: Option<f64>,
    #[serde(default)]
    pub star_line: Option<f64>,
    #[serde(default)]
    pub bull_line: Option<f64>,
    #[serde(default)]
    pub wait_line: Option<f64>,
    #[serde(default)]
    pub support: Option<f64>,
    #[serde(default)]
    pub resistance: Option<f64>,
    #[serde(default)]
    pub breakout: Option<f64>,
    #[serde(default)]
    pub reversal: Option<f64>,
    #[serde(default)]
    pub swl_above_sws: bool,
    #[serde(default)]
    pub kdj_golden_cross: bool,
    #[serde(default)]
    pub kdj_dead_cross: bool,
    #[serde(default)]
    pub kdj_overbought: bool,
    #[serde(default)]
    pub kdj_oversold: bool,
    #[serde(default)]
    pub red_hold: bool,
    #[serde(default)]
    pub cyan_watch: bool,
    #[serde(default)]
    pub short_buy: bool,
    #[serde(default)]
    pub white_exit: bool,
    #[serde(default)]
    pub oversold: bool,
    #[serde(default = "default_trend_signal_type")]
    pub signal_type: String,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technical_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_layer_score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f64>,
    #[serde(default)]
    pub quant_score: i32,
    #[serde(default = "default_quant_score_max")]
    pub quant_score_max: i32,
    #[serde(default)]
    pub pattern_score: i32,
    #[serde(default = "default_pattern_score_max")]
    pub pattern_score_max: i32,
    #[serde(default)]
    pub pattern_signals: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendIndicatorResult {
    pub stock: StockItem,
    pub signal: TrendIndicatorSignal,
    #[serde(default)]
    pub series: Vec<TrendIndicatorPoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockObserveRequest {
    pub code: String,
    #[serde(default = "default_start_date")]
    pub start_date: String,
    #[serde(default = "default_end_date")]
    pub end_date: String,
    #[serde(default = "default_series_limit")]
    pub series_limit: usize,
    #[serde(default)]
    pub include_order_book: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinancialIndicatorItem {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default = "default_tone")]
    pub tone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinancialIndicators {
    pub title: String,
    pub period: String,
    pub source: String,
    pub items: Vec<FinancialIndicatorItem>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalEvidenceItem {
    pub category: String,
    pub source: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_metrics")]
    pub metrics: BTreeMap<String, String>,
    #[serde(default = "default_uncertain_sentiment")]
    pub sentiment: String,
    #[serde(default)]
    pub weight: f64,
    #[serde(default = "default_low_confidence")]
    pub confidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn deserialize_string_metrics<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<BTreeMap<String, Value>>::deserialize(deserializer)?;
    let Some(map) = value else {
        return Ok(BTreeMap::new());
    };
    let mut metrics = BTreeMap::new();
    for (key, value) in map {
        let text = match value {
            Value::Null => String::new(),
            Value::String(value) => value,
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            other => other.to_string(),
        };
        metrics.insert(key, text);
    }
    Ok(metrics)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalEvidenceSection {
    pub key: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default)]
    pub weight: f64,
    #[serde(default)]
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub items: Vec<CapitalEvidenceItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalEvidenceResult {
    pub stock_code: String,
    pub generated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composite_score: Option<f64>,
    #[serde(default = "default_low_confidence")]
    pub confidence: String,
    #[serde(default)]
    pub model_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of_trade_date: Option<String>,
    #[serde(default = "default_unknown_freshness")]
    pub freshness: String,
    #[serde(default)]
    pub contributions: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub sections: Vec<CapitalEvidenceSection>,
    #[serde(default)]
    pub items: Vec<CapitalEvidenceItem>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockObservation {
    pub source: String,
    pub stock: StockItem,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub financial_indicators: Option<FinancialIndicators>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trend: Option<TrendIndicatorResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital_evidence: Option<CapitalEvidenceResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_book: Option<Value>,
    #[serde(default)]
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendScreenRequest {
    #[serde(default)]
    pub criteria: ScreenCriteria,
    #[serde(default = "default_start_date")]
    pub start_date: String,
    #[serde(default = "default_end_date")]
    pub end_date: String,
    #[serde(default = "default_trend_limit")]
    pub limit: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendStockSignal {
    pub stock: StockItem,
    pub base_score: f64,
    pub trend_score: f64,
    pub final_score: f64,
    pub signal: TrendIndicatorSignal,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub explanation: SelectionExplanation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendScreenResult {
    pub total: usize,
    pub returned: usize,
    pub items: Vec<TrendStockSignal>,
    #[serde(default = "default_trend_screen_style")]
    pub screen_style: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestRequest {
    #[serde(default)]
    pub algorithm_version: Option<String>,
    #[serde(default)]
    pub parameter_set_id: Option<String>,
    #[serde(default)]
    pub scoring_parameters: Option<ScoringParameters>,
    #[serde(default)]
    pub criteria: ScreenCriteria,
    #[serde(default = "default_backtest_source")]
    pub source: String,
    #[serde(default = "default_backtest_strategy_mode")]
    pub strategy_mode: String,
    #[serde(default)]
    pub stock_codes: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    #[serde(default = "default_top_n")]
    pub top_n: usize,
    #[serde(default = "default_initial_cash")]
    pub initial_cash: f64,
    #[serde(default = "default_rebalance_frequency")]
    pub rebalance_frequency: String,
    #[serde(default = "default_transaction_cost_bps")]
    pub transaction_cost_bps: f64,
    #[serde(default = "default_backtest_benchmark")]
    pub benchmark: String,
}

impl Default for BacktestRequest {
    fn default() -> Self {
        Self {
            criteria: ScreenCriteria::default(),
            source: default_backtest_source(),
            strategy_mode: default_backtest_strategy_mode(),
            algorithm_version: None,
            parameter_set_id: None,
            scoring_parameters: None,
            stock_codes: Vec::new(),
            start_date: String::new(),
            end_date: String::new(),
            top_n: default_top_n(),
            initial_cash: default_initial_cash(),
            rebalance_frequency: default_rebalance_frequency(),
            transaction_cost_bps: default_transaction_cost_bps(),
            benchmark: default_backtest_benchmark(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EquityPoint {
    pub date: String,
    pub equity: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub total_return: f64,
    pub annualized_return: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub num_stocks: usize,
    pub benchmark_total_return: Option<f64>,
    pub benchmark_annualized_return: Option<f64>,
    pub benchmark_max_drawdown: Option<f64>,
    pub excess_return: Option<f64>,
    pub total_transaction_cost: f64,
    pub total_turnover: f64,
    pub rebalance_count: usize,
    #[serde(default)]
    pub oos_fold_count: usize,
    #[serde(default)]
    pub evaluated_selection_count: usize,
    #[serde(default)]
    pub selection_hit_count: usize,
    #[serde(default)]
    pub precision_at_n: Option<f64>,
    #[serde(default = "default_backtest_strategy_mode")]
    pub strategy_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalkForwardFold {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_date: Option<String>,
    pub selection_date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_end_date: Option<String>,
    #[serde(default)]
    pub selected_symbols: Vec<String>,
    #[serde(default)]
    pub eligible_symbol_count: usize,
    #[serde(default)]
    pub evaluated_selection_count: usize,
    #[serde(default)]
    pub hit_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_at_n: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_forward_return: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_forward_return: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_excess_return: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestResult {
    pub metrics: BacktestMetrics,
    pub equity_curve: Vec<EquityPoint>,
    #[serde(default)]
    pub benchmark_curve: Vec<EquityPoint>,
    pub symbols: Vec<String>,
    #[serde(default)]
    pub benchmark_symbols: Vec<String>,
    #[serde(default)]
    pub rebalance_dates: Vec<String>,
    #[serde(default)]
    pub walk_forward_folds: Vec<WalkForwardFold>,
    #[serde(default)]
    pub volatility_snapshots: Vec<VolatilitySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatility_message: Option<String>,
    #[serde(default = "default_backtest_strategy_mode")]
    pub strategy_mode: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AdaptiveReleaseGateInput {
    pub release_configuration_qualified: Option<bool>,
    pub legacy_annualized_return: Option<f64>,
    pub adaptive_annualized_return: Option<f64>,
    pub legacy_max_drawdown: Option<f64>,
    pub adaptive_max_drawdown: Option<f64>,
    pub legacy_precision_at_10: Option<f64>,
    pub adaptive_precision_at_10: Option<f64>,
    pub max_primary_industry_count: Option<usize>,
    pub average_adjacent_jaccard: Option<f64>,
    pub five_run_unique_coverage: Option<usize>,
    pub first_run_millis: Option<u64>,
    pub cached_run_millis: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveReleaseGateCheck {
    pub key: String,
    pub passed: bool,
    pub actual: Option<f64>,
    pub requirement: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveReleaseGateReport {
    pub passed: bool,
    pub checks: Vec<AdaptiveReleaseGateCheck>,
}

pub fn evaluate_adaptive_release_gate(
    input: &AdaptiveReleaseGateInput,
) -> AdaptiveReleaseGateReport {
    let mut checks = Vec::new();
    let mut push = |key: &str, actual: Option<f64>, passed: bool, requirement: &str| {
        checks.push(AdaptiveReleaseGateCheck {
            key: key.to_string(),
            passed,
            actual,
            requirement: requirement.to_string(),
        });
    };
    push(
        "release_configuration_qualified",
        input
            .release_configuration_qualified
            .map(|value| if value { 1.0 } else { 0.0 }),
        input.release_configuration_qualified == Some(true),
        "auto mode, full universe, top 10, monthly, >=60 strict out-of-sample folds",
    );
    let return_delta = input
        .adaptive_annualized_return
        .zip(input.legacy_annualized_return)
        .map(|(adaptive, legacy)| adaptive - legacy);
    push(
        "annualized_return_delta",
        return_delta,
        return_delta.is_some_and(|value| value + 1e-12 >= -0.01),
        "adaptive annualized return no worse than legacy by more than 1 percentage point",
    );
    let drawdown_delta = input
        .adaptive_max_drawdown
        .zip(input.legacy_max_drawdown)
        .map(|(adaptive, legacy)| adaptive - legacy);
    push(
        "max_drawdown_delta",
        drawdown_delta,
        drawdown_delta.is_some_and(|value| value + 1e-12 >= -0.01),
        "adaptive drawdown no worse than legacy by more than 1 percentage point",
    );
    let precision_delta = input
        .adaptive_precision_at_10
        .zip(input.legacy_precision_at_10)
        .map(|(adaptive, legacy)| adaptive - legacy);
    push(
        "precision_at_10_delta",
        precision_delta,
        precision_delta.is_some_and(|value| value + 1e-12 >= -0.03),
        "adaptive Precision@10 no worse than legacy by more than 3 percentage points",
    );
    push(
        "max_primary_industry_count",
        input.max_primary_industry_count.map(|value| value as f64),
        input
            .max_primary_industry_count
            .is_some_and(|value| value <= 3),
        "no primary industry exceeds 3/10",
    );
    push(
        "average_adjacent_jaccard",
        input.average_adjacent_jaccard,
        input
            .average_adjacent_jaccard
            .is_some_and(|value| value <= 0.70),
        "average adjacent primary Jaccard <= 0.70",
    );
    let coverage = input.five_run_unique_coverage.map(|value| value as f64);
    push(
        "five_run_unique_coverage",
        coverage,
        input
            .five_run_unique_coverage
            .is_some_and(|value| value >= 20),
        "five consecutive primary+exploration runs cover at least 20 symbols",
    );
    let first_run = input.first_run_millis.map(|value| value as f64);
    push(
        "first_run_millis",
        first_run,
        input.first_run_millis.is_some_and(|value| value <= 20_000),
        "first network run <= 20000 ms",
    );
    let cached_run = input.cached_run_millis.map(|value| value as f64);
    push(
        "cached_run_millis",
        cached_run,
        input.cached_run_millis.is_some_and(|value| value <= 2_000),
        "same-day cached run <= 2000 ms",
    );
    AdaptiveReleaseGateReport {
        passed: checks.iter().all(|check| check.passed),
        checks,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRequest {
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSkill {
    pub key: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub platforms: Vec<String>,
    pub evidence_level: String,
    pub fallback: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentIntent {
    pub kind: String,
    pub query: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub window: Option<String>,
    pub depth: String,
    pub mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentToolCall {
    pub id: String,
    pub tool: String,
    pub label: String,
    pub status: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub output_summary: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEvidenceItem {
    pub title: String,
    pub source: String,
    pub level: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentAnswerSection {
    pub title: String,
    #[serde(default)]
    pub bullets: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    #[serde(default)]
    pub watchlist: Vec<AgentWatchlistItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentWatchlistItem {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub industry: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentResponse {
    pub reply: String,
    pub action: String,
    #[serde(default)]
    pub intent: Option<AgentIntent>,
    #[serde(default)]
    pub tool_calls: Vec<AgentToolCall>,
    #[serde(default)]
    pub evidence_summary: Vec<AgentEvidenceItem>,
    #[serde(default)]
    pub answer_sections: Vec<AgentAnswerSection>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub criteria: Option<ScreenCriteria>,
    #[serde(default)]
    pub backtest: Option<BacktestRequest>,
    #[serde(default)]
    pub graph_screen: Option<GraphScreenRequest>,
    #[serde(default)]
    pub trend_screen: Option<TrendScreenRequest>,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryBar {
    pub date: String,
    #[serde(default)]
    pub open: Option<f64>,
    #[serde(default)]
    pub high: Option<f64>,
    #[serde(default)]
    pub low: Option<f64>,
    pub close: f64,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub capital: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuarterlyEpsPoint {
    pub period: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockFinancialSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_eps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_bps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_revenue_billion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_revenue_yoy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_net_profit_billion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_net_profit_yoy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gross_margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roe: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_liability_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goodwill_to_net_assets: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pledged_share_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_yield: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_payout_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goodwill_period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pledged_share_period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub quarterly_eps: Vec<QuarterlyEpsPoint>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockFactorSnapshot {
    pub date: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub industry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_st: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_listed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_tradable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roe: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_cap_billion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividend_yield: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deducted_net_profit_billion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deducted_net_profit_margin: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deducted_net_profit_growth_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turnover_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CoreDataSet {
    #[serde(default)]
    pub stocks: Vec<StockItem>,
    #[serde(default)]
    pub relations: Vec<StockRelation>,
    #[serde(default)]
    pub histories: HashMap<String, Vec<HistoryBar>>,
    #[serde(default)]
    pub financials: HashMap<String, StockFinancialSnapshot>,
    #[serde(default)]
    pub factor_snapshots: HashMap<String, Vec<StockFactorSnapshot>>,
    #[serde(default)]
    pub capital_evidence: HashMap<String, CapitalEvidenceResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSourceSummary {
    pub stock_count: usize,
    pub relation_count: usize,
    pub history_symbol_count: usize,
    pub history_bar_count: usize,
    #[serde(default)]
    pub factor_snapshot_symbol_count: usize,
    #[serde(default)]
    pub factor_snapshot_count: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenWithDataRequest {
    pub data: CoreDataSet,
    #[serde(default)]
    pub criteria: ScreenCriteria,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorScreenWithDataRequest {
    pub data: CoreDataSet,
    #[serde(default)]
    pub request: SectorScreenRequest,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphScreenWithDataRequest {
    pub data: CoreDataSet,
    #[serde(default)]
    pub request: GraphScreenRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestWithDataRequest {
    pub data: CoreDataSet,
    pub request: BacktestRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendWithDataRequest {
    pub data: CoreDataSet,
    pub request: TrendIndicatorRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserveWithDataRequest {
    pub data: CoreDataSet,
    pub request: StockObserveRequest,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrendScreenWithDataRequest {
    pub data: CoreDataSet,
    pub request: TrendScreenRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWithDataRequest {
    pub data: CoreDataSet,
    pub message: String,
    #[serde(default)]
    pub context: AgentContext,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStreamWithDataRequest {
    pub data: CoreDataSet,
    pub message: String,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub context: AgentContext,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStreamEvent {
    pub run_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<AgentResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileStockSourceItem {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub source_tier: String,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileStockSkillRequest {
    pub stock_code: String,
    #[serde(default)]
    pub stock_name: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub sources: Vec<MobileStockSourceItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileStockSkillFinding {
    pub label: String,
    pub title: String,
    pub summary: String,
    pub source_tier: String,
    pub source_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    pub evidence: String,
    pub confidence: f64,
    pub risk_note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileStockSkillOverview {
    pub stock_code: String,
    pub stock_name: String,
    pub overall_label: String,
    pub summary: String,
    pub positive_count: usize,
    pub negative_count: usize,
    pub neutral_count: usize,
    pub unverified_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MobileStockSkillResult {
    pub overview: MobileStockSkillOverview,
    #[serde(default)]
    pub positive_factors: Vec<MobileStockSkillFinding>,
    #[serde(default)]
    pub negative_factors: Vec<MobileStockSkillFinding>,
    #[serde(default)]
    pub neutral_information: Vec<MobileStockSkillFinding>,
    #[serde(default)]
    pub unverified_leads: Vec<MobileStockSkillFinding>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug)]
struct HistoryPoint {
    date: NaiveDate,
    close: f64,
}

#[derive(Clone, Debug)]
struct PreparedBar {
    date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    capital: f64,
}

#[derive(Clone, Debug)]
struct ComputedTrendBar {
    date: NaiveDate,
    close: f64,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    previous_close: Option<f64>,
    close_change: Option<f64>,
    close_change_pct: Option<f64>,
    swl: f64,
    sws: f64,
    k: f64,
    d: f64,
    j: f64,
    accumulation_index: f64,
    accumulation_strength: f64,
    swing_opportunity: f64,
    rebound_signal: f64,
    trend_heat: f64,
    volume_price_heat: f64,
    anomaly_heat: f64,
    popularity_heat: f64,
    star_line: f64,
    bull_line: f64,
    wait_line: f64,
    support: f64,
    resistance: f64,
    breakout: f64,
    reversal: f64,
    swl_above_sws: bool,
    kdj_golden_cross: bool,
    kdj_dead_cross: bool,
    kdj_overbought: bool,
    kdj_oversold: bool,
    red_hold: bool,
    cyan_watch: bool,
    short_buy: bool,
    white_exit: bool,
    oversold: bool,
    quant_score: i32,
}

pub trait MarketDataSource {
    fn stocks(&self) -> CoreResult<&[StockItem]>;

    fn relations(&self) -> CoreResult<&[StockRelation]> {
        Ok(&[])
    }

    fn get_history(
        &self,
        code: &str,
        start_date: &str,
        end_date: &str,
    ) -> CoreResult<Vec<HistoryBar>>;

    fn get_financial_snapshot(&self, _code: &str) -> Option<StockFinancialSnapshot> {
        None
    }

    fn get_factor_snapshots(&self, _code: &str) -> CoreResult<Vec<StockFactorSnapshot>> {
        Ok(Vec::new())
    }

    fn factor_snapshot_codes(&self) -> CoreResult<Vec<String>> {
        Ok(Vec::new())
    }

    fn get_capital_evidence(&self, _code: &str) -> Option<CapitalEvidenceResult> {
        None
    }
}

pub struct MockDataSource;

impl MarketDataSource for MockDataSource {
    fn stocks(&self) -> CoreResult<&[StockItem]> {
        static STOCKS: OnceLock<Vec<StockItem>> = OnceLock::new();
        Ok(STOCKS.get_or_init(mock_stocks).as_slice())
    }

    fn relations(&self) -> CoreResult<&[StockRelation]> {
        static RELATIONS: OnceLock<Vec<StockRelation>> = OnceLock::new();
        Ok(RELATIONS.get_or_init(mock_relations).as_slice())
    }

    fn get_history(
        &self,
        code: &str,
        start_date: &str,
        end_date: &str,
    ) -> CoreResult<Vec<HistoryBar>> {
        mock_history(&mock_stocks(), code, start_date, end_date)
    }
}

pub struct StaticDataSource<'a> {
    data: &'a CoreDataSet,
    stock_override: Option<&'a [StockItem]>,
    history_override: Option<&'a HashMap<String, Vec<HistoryBar>>>,
}

impl<'a> StaticDataSource<'a> {
    pub fn new(data: &'a CoreDataSet) -> Self {
        Self {
            data,
            stock_override: None,
            history_override: None,
        }
    }

    pub fn with_stocks(data: &'a CoreDataSet, stocks: &'a [StockItem]) -> Self {
        Self {
            data,
            stock_override: Some(stocks),
            history_override: None,
        }
    }

    pub fn with_overrides(
        data: &'a CoreDataSet,
        stocks: Option<&'a [StockItem]>,
        histories: Option<&'a HashMap<String, Vec<HistoryBar>>>,
    ) -> Self {
        Self {
            data,
            stock_override: stocks,
            history_override: histories,
        }
    }
}

impl MarketDataSource for StaticDataSource<'_> {
    fn stocks(&self) -> CoreResult<&[StockItem]> {
        Ok(self.stock_override.unwrap_or(&self.data.stocks))
    }

    fn relations(&self) -> CoreResult<&[StockRelation]> {
        Ok(&self.data.relations)
    }

    fn get_history(
        &self,
        code: &str,
        start_date: &str,
        end_date: &str,
    ) -> CoreResult<Vec<HistoryBar>> {
        let normalized = normalize_stock_code(code);
        if let Some(history) = self
            .history_override
            .and_then(|histories| histories.get(code).or_else(|| histories.get(&normalized)))
        {
            return history_bars_in_range(history, start_date, end_date);
        }
        let Some(history) = self
            .data
            .histories
            .get(code)
            .or_else(|| self.data.histories.get(&normalized))
        else {
            return Ok(Vec::new());
        };
        history_bars_in_range(history, start_date, end_date)
    }

    fn get_financial_snapshot(&self, code: &str) -> Option<StockFinancialSnapshot> {
        let normalized = normalize_stock_code(code);
        self.data
            .financials
            .get(code)
            .cloned()
            .or_else(|| self.data.financials.get(&normalized).cloned())
    }

    fn get_factor_snapshots(&self, code: &str) -> CoreResult<Vec<StockFactorSnapshot>> {
        let normalized = normalize_stock_code(code);
        Ok(self
            .data
            .factor_snapshots
            .get(code)
            .cloned()
            .or_else(|| self.data.factor_snapshots.get(&normalized).cloned())
            .unwrap_or_default())
    }

    fn factor_snapshot_codes(&self) -> CoreResult<Vec<String>> {
        let mut codes = self
            .data
            .factor_snapshots
            .keys()
            .map(|code| normalize_stock_code(code))
            .collect::<Vec<_>>();
        codes.sort();
        codes.dedup();
        Ok(codes)
    }

    fn get_capital_evidence(&self, code: &str) -> Option<CapitalEvidenceResult> {
        let normalized = normalize_stock_code(code);
        self.data
            .capital_evidence
            .get(code)
            .cloned()
            .or_else(|| self.data.capital_evidence.get(&normalized).cloned())
    }
}

fn default_screen_limit() -> usize {
    10
}

fn default_sort_by() -> String {
    "score".to_string()
}

fn default_sort_dir() -> String {
    "desc".to_string()
}

fn default_score_profile() -> String {
    "balanced".to_string()
}

fn default_relation_weight() -> f64 {
    1.0
}

fn default_relation_depth() -> usize {
    1
}

fn default_relation_weight_param() -> f64 {
    0.35
}

fn default_tone() -> String {
    "neutral".to_string()
}

fn default_uncertain_sentiment() -> String {
    "uncertain".to_string()
}

fn default_low_confidence() -> String {
    "\u{4f4e}".to_string()
}

fn default_unknown_freshness() -> String {
    "unknown".to_string()
}

fn default_graph_center_mode() -> String {
    "seed_codes".to_string()
}

fn default_trend_screen_style() -> String {
    "balanced_swing".to_string()
}

fn default_trend_signal_type() -> String {
    "trend_continuation".to_string()
}

fn default_graph_limit() -> usize {
    10
}

fn default_sector_group_limit() -> usize {
    12
}

fn default_per_sector_limit() -> usize {
    5
}

fn default_min_sector_candidates() -> usize {
    5
}

fn default_sector_group_by() -> String {
    "concept".to_string()
}

fn default_start_date() -> String {
    "20200101".to_string()
}

fn default_end_date() -> String {
    current_system_date_yyyymmdd()
}

fn current_system_date_yyyymmdd() -> String {
    Local::now().date_naive().format("%Y%m%d").to_string()
}

fn default_series_limit() -> usize {
    120
}

fn clamp_series_limit(limit: usize) -> usize {
    limit.clamp(20, 10_000)
}

fn default_trend_limit() -> usize {
    10
}

fn default_quant_score_max() -> i32 {
    90
}

fn default_pattern_score_max() -> i32 {
    100
}

fn default_top_n() -> usize {
    10
}

fn default_initial_cash() -> f64 {
    1_000_000.0
}

fn default_rebalance_frequency() -> String {
    "monthly".to_string()
}

fn default_transaction_cost_bps() -> f64 {
    10.0
}

fn default_backtest_benchmark() -> String {
    "candidate_equal_weight".to_string()
}

fn default_backtest_strategy_mode() -> String {
    "walk_forward".to_string()
}

fn default_backtest_source() -> String {
    "criteria".to_string()
}

pub fn screen_value(payload: Value) -> CoreResult<Value> {
    let criteria: ScreenCriteria = serde_json::from_value(payload)?;
    serde_json::to_value(screen_with_mock(&criteria)).map_err(Into::into)
}

pub fn sector_screen_value(payload: Value) -> CoreResult<Value> {
    let request: SectorScreenRequest = serde_json::from_value(payload)?;
    serde_json::to_value(sector_screen_with_mock(&request)).map_err(Into::into)
}
pub fn screen_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: ScreenWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(screen_with_data(&request.data, &request.criteria)?).map_err(Into::into)
}

pub fn sector_screen_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: SectorScreenWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(sector_screen_with_data(&request.data, &request.request)?)
        .map_err(Into::into)
}
pub fn graph_screen_value(payload: Value) -> CoreResult<Value> {
    let request: GraphScreenRequest = serde_json::from_value(payload)?;
    serde_json::to_value(graph_screen_with_mock(&request)).map_err(Into::into)
}

pub fn graph_screen_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: GraphScreenWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(graph_screen_with_data(&request.data, &request.request)?)
        .map_err(Into::into)
}

pub fn backtest_value(payload: Value) -> CoreResult<Value> {
    let request: BacktestRequest = serde_json::from_value(payload)?;
    serde_json::to_value(backtest_with_mock(&request)?).map_err(Into::into)
}

pub fn backtest_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: BacktestWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(backtest_with_data(&request.data, &request.request)?).map_err(Into::into)
}

pub fn trend_value(payload: Value) -> CoreResult<Value> {
    let request: TrendIndicatorRequest = serde_json::from_value(payload)?;
    serde_json::to_value(trend_with_mock(&request)?).map_err(Into::into)
}

pub fn observe_value(payload: Value) -> CoreResult<Value> {
    let request: StockObserveRequest = serde_json::from_value(payload)?;
    serde_json::to_value(observe_with_mock(&request)?).map_err(Into::into)
}
pub fn trend_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: TrendWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(trend_with_data(&request.data, &request.request)?).map_err(Into::into)
}

pub fn observe_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: ObserveWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(observe_with_data(&request.data, &request.request)?).map_err(Into::into)
}
pub fn trend_screen_value(payload: Value) -> CoreResult<Value> {
    let request: TrendScreenRequest = serde_json::from_value(payload)?;
    serde_json::to_value(trend_screen_with_mock(&request)?).map_err(Into::into)
}

pub fn trend_screen_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: TrendScreenWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(trend_screen_with_data(&request.data, &request.request)?)
        .map_err(Into::into)
}

pub fn agent_value(payload: Value) -> CoreResult<Value> {
    let request: AgentRequest = serde_json::from_value(payload)?;
    serde_json::to_value(run_agent_with_mock(&request.message)?).map_err(Into::into)
}

pub fn agent_with_data_value(payload: Value) -> CoreResult<Value> {
    let request: AgentWithDataRequest = serde_json::from_value(payload)?;
    serde_json::to_value(run_agent_with_data_and_context(
        &request.data,
        &request.message,
        &request.context,
    )?)
    .map_err(Into::into)
}

pub fn agent_stream_with_data_events_value(payload: Value) -> CoreResult<Vec<AgentStreamEvent>> {
    let request: AgentStreamWithDataRequest = serde_json::from_value(payload)?;
    Ok(run_agent_stream_with_data_events_with_context(
        &request.data,
        &request.message,
        request.run_id.as_deref(),
        request.mode.as_deref(),
        &request.context,
    ))
}

pub fn mobile_stock_skill_value(payload: Value) -> CoreResult<Value> {
    let request: MobileStockSkillRequest = serde_json::from_value(payload)?;
    serde_json::to_value(run_mobile_stock_skill(&request)).map_err(Into::into)
}

pub fn validate_data_source_value(payload: Value) -> CoreResult<Value> {
    let data: CoreDataSet = serde_json::from_value(payload)?;
    serde_json::to_value(validate_data_set(&data)?).map_err(Into::into)
}

pub fn screen_with_mock(criteria: &ScreenCriteria) -> ScreenResult {
    screen_with_source(&MockDataSource, criteria).expect("mock data source should be valid")
}

pub fn sector_screen_with_mock(request: &SectorScreenRequest) -> SectorScreenResult {
    let universe = mock_stocks();
    sector_screen_stocks(&universe, request)
}
pub fn screen_with_data(data: &CoreDataSet, criteria: &ScreenCriteria) -> CoreResult<ScreenResult> {
    let source = StaticDataSource::new(data);
    screen_with_source(&source, criteria)
}

pub fn screen_with_source(
    source: &impl MarketDataSource,
    criteria: &ScreenCriteria,
) -> CoreResult<ScreenResult> {
    let universe = source.stocks()?;
    Ok(screen_stocks(universe, criteria))
}

pub fn sector_screen_with_data(
    data: &CoreDataSet,
    request: &SectorScreenRequest,
) -> CoreResult<SectorScreenResult> {
    let source = StaticDataSource::new(data);
    let universe = source.stocks()?;
    Ok(sector_screen_stocks(universe, request))
}

pub fn sector_screen_stocks(
    universe: &[StockItem],
    request: &SectorScreenRequest,
) -> SectorScreenResult {
    let max_sectors = request.max_sectors.clamp(1, 50);
    let per_sector_limit = request.per_sector_limit.clamp(1, 50);
    let min_sector_candidates = request.min_sector_candidates.clamp(1, 500);
    let group_by = request.group_by.trim().to_ascii_lowercase();
    let board_mode = matches!(group_by.as_str(), "board" | "market" | "market_board");
    let mut screened = Vec::new();
    let mut notes = deducted_profit_rule_notes(universe, &request.criteria);

    for stock in universe {
        let Some(reasons) = matches_stock(stock, &request.criteria) else {
            continue;
        };
        screened.push(score_stock(
            stock,
            &reasons,
            &request.criteria.score_profile,
        ));
    }
    sort_screened(&mut screened, &request.criteria);

    let mut by_sector: HashMap<String, Vec<ScreenedStock>> = HashMap::new();
    for item in &screened {
        let sector = if board_mode {
            board_group_for_stock(&item.stock).to_string()
        } else {
            concept_sector_group_for_stock(&item.stock)
        };
        by_sector.entry(sector).or_default().push(item.clone());
    }

    let omitted_small_sector_count = by_sector
        .values()
        .filter(|items| items.len() < min_sector_candidates)
        .count();
    let omitted_small_stock_count = by_sector
        .values()
        .filter(|items| items.len() < min_sector_candidates)
        .map(Vec::len)
        .sum::<usize>();

    let mut groups: Vec<SectorScreenGroup> = by_sector
        .into_iter()
        .filter(|(_, items)| items.len() >= min_sector_candidates)
        .map(|(sector, mut items)| {
            let total = items.len();
            items.truncate(per_sector_limit);
            let returned = items.len();
            let average_score = if returned == 0 {
                0.0
            } else {
                items.iter().map(|item| item.score).sum::<f64>() / returned as f64
            };
            SectorScreenGroup {
                sector,
                total,
                returned,
                average_score: (average_score * 1_000_000.0).round() / 1_000_000.0,
                items,
            }
        })
        .collect();
    groups.sort_by(|left, right| {
        let left_rank = if board_mode {
            board_rank(&left.sector)
        } else {
            concept_rank(&left.sector)
        };
        let right_rank = if board_mode {
            board_rank(&right.sector)
        } else {
            concept_rank(&right.sector)
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.average_score.total_cmp(&left.average_score))
            .then_with(|| right.total.cmp(&left.total))
            .then_with(|| left.sector.cmp(&right.sector))
    });
    groups.truncate(max_sectors);
    let returned = groups.iter().map(|group| group.returned).sum();
    if request
        .criteria
        .industry
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        notes.push(format!(
            "{}{}",
            "\u{5df2}\u{9650}\u{5b9a}\u{884c}\u{4e1a}\u{ff1a}",
            request.criteria.industry.as_deref().unwrap_or_default()
        ));
    }
    if omitted_small_sector_count > 0 {
        notes.push(format!(
            "已隐藏 {omitted_small_sector_count} 个样本不足的行业分组（共 {omitted_small_stock_count} 只股票）；分组门槛为 {min_sector_candidates} 只。"
        ));
    }
    notes.push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
    SectorScreenResult {
        total: screened.len(),
        returned,
        sector_count: groups.len(),
        groups,
        notes,
    }
}
pub fn graph_screen_with_mock(request: &GraphScreenRequest) -> GraphScreenResult {
    graph_screen_with_source(&MockDataSource, request).expect("mock data source should be valid")
}

pub fn graph_screen_with_data(
    data: &CoreDataSet,
    request: &GraphScreenRequest,
) -> CoreResult<GraphScreenResult> {
    let source = StaticDataSource::new(data);
    graph_screen_with_source(&source, request)
}

pub fn graph_screen_with_source(
    source: &impl MarketDataSource,
    request: &GraphScreenRequest,
) -> CoreResult<GraphScreenResult> {
    let universe = source.stocks()?;
    let relations = source.relations()?;
    Ok(graph_screen_stocks(universe, relations, request))
}

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

struct BacktestDataRequirements {
    universe: Vec<StockItem>,
    selection_notes: Vec<String>,
    symbols: Vec<String>,
    history_symbols: Vec<String>,
}

fn resolve_backtest_data_requirements(
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
struct BacktestHistory {
    code: String,
    prices: BTreeMap<NaiveDate, f64>,
    bars: Vec<HistoryBar>,
}

#[derive(Clone, Debug)]
struct PortfolioSimulation {
    equity_curve: Vec<EquityPoint>,
    rebalance_dates: Vec<NaiveDate>,
    total_transaction_cost: f64,
    total_turnover: f64,
    selections: Vec<RebalanceSelection>,
}

#[derive(Clone, Debug)]
struct ActiveSelection {
    selected_indices: Vec<usize>,
    eligible_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
struct RebalanceSelection {
    signal_date: NaiveDate,
    date: NaiveDate,
    selected_indices: Vec<usize>,
    eligible_indices: Vec<usize>,
}

fn empty_backtest_metrics(num_stocks: usize, strategy_mode: &str) -> BacktestMetrics {
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

fn strict_walk_forward_history_symbols(
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

fn load_backtest_histories(
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

fn simulate_equal_weight_portfolio(
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

fn simulate_walk_forward_portfolio(
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

const MIN_HISTORY_BARS_FOR_ADAPTIVE_BACKTEST: usize = 60;

#[allow(clippy::too_many_arguments)] // 内部模拟函数，参数即上下文，拆结构体反而更难读
fn simulate_walk_forward_portfolio_with_snapshots(
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

fn load_factor_snapshot_timeline(
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

fn walk_forward_active_indices(
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
fn adaptive_walk_forward_active_indices(
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

fn point_in_time_universe(
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

fn latest_factor_snapshot_on_or_before<'a>(
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

fn apply_factor_snapshot(
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

fn equal_weight_benchmark_curve(
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

fn backtest_calendar(histories: &[BacktestHistory]) -> Vec<NaiveDate> {
    let mut dates = BTreeMap::new();
    for history in histories {
        for date in history.prices.keys() {
            dates.insert(*date, ());
        }
    }
    dates.into_keys().collect()
}

fn latest_price_on_or_before(history: &BacktestHistory, date: NaiveDate) -> Option<f64> {
    history
        .prices
        .range(..=date)
        .next_back()
        .map(|(_, price)| *price)
}

fn price_on_date(history: &BacktestHistory, date: NaiveDate) -> Option<f64> {
    history.prices.get(&date).copied()
}

fn selected_symbols_from_latest_rebalance(
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

fn selected_symbols_from_rebalances(
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

fn evaluate_walk_forward_folds(
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

fn forward_return(history: &BacktestHistory, start: NaiveDate, end: NaiveDate) -> Option<f64> {
    let start_price = price_on_date(history, start)?;
    let end_price = price_on_date(history, end)?;
    (start_price > 0.0 && end_price > 0.0).then_some(end_price / start_price - 1.0)
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then_some(values.iter().sum::<f64>() / values.len() as f64)
}

fn should_rebalance(
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

fn quarter(date: NaiveDate) -> u32 {
    (date.month0() / 3) + 1
}

fn sanitize_initial_cash(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        default_initial_cash()
    }
}

fn sanitize_transaction_cost_rate(bps: f64) -> f64 {
    if bps.is_finite() {
        bps.clamp(0.0, 500.0) / 10_000.0
    } else {
        default_transaction_cost_bps() / 10_000.0
    }
}

fn normalize_rebalance_frequency(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => "none".to_string(),
        "quarterly" => "quarterly".to_string(),
        _ => "monthly".to_string(),
    }
}

fn normalize_backtest_benchmark(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("none") {
        "none".to_string()
    } else {
        "candidate_equal_weight".to_string()
    }
}

fn normalize_backtest_strategy_mode(value: &str) -> String {
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

fn backtest_strategy_mode(request: &BacktestRequest) -> String {
    request
        .algorithm_version
        .clone()
        .unwrap_or_else(|| normalize_backtest_strategy_mode(&request.strategy_mode))
}

fn is_adaptive_backtest(request: &BacktestRequest) -> bool {
    matches!(
        backtest_strategy_mode(request).as_str(),
        "adaptive_swing_v1" | "adaptive_swing_v2"
    )
}

fn backtest_scoring_parameters(request: &BacktestRequest) -> CoreResult<Option<ScoringParameters>> {
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

fn adaptive_backtest_requested_mode(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_once(':')
        .map(|(_, mode)| mode)
        .filter(|mode| matches!(*mode, "auto" | "range" | "trend" | "defensive"))
        .unwrap_or("auto")
        .to_string()
}

fn backtest_strategy_mode_note(value: &str) -> &'static str {
    match value {
        "adaptive_swing_v2" => "策略模式：adaptive_swing_v2；使用经校验的冻结参数及前一交易日可见数据生成信号，在下一交易日按实际报价调仓。",
        "adaptive_swing_v1" => "策略模式：adaptive_swing_v1；使用前一交易日收盘前可见的财务快照、OHLCV、三宽基指数和市场宽度生成信号，并在下一交易日按实际报价调仓。",
        "walk_forward" => "策略模式：walk_forward，仅使用成交日前一交易日已公开的因子快照和历史上市/ST/可交易状态；成交只使用执行日实际报价。",
        _ => "策略模式：candidate_snapshot，展示当前候选池的历史组合表现，不等同于严格可交易的逐日选股策略。",
    }
}

fn backtest_rebalance_note(value: &str) -> &'static str {
    match value {
        "none" => "买入持有",
        "quarterly" => "季度调仓",
        _ => "月度调仓",
    }
}
fn selected_backtest_items(
    universe: &[StockItem],
    request: &BacktestRequest,
) -> (Vec<ScreenedStock>, Vec<String>) {
    selected_backtest_items_from_universe(universe, request)
}

fn selected_backtest_items_from_universe(
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

fn normalize_backtest_source(source: &str) -> String {
    if source.trim().eq_ignore_ascii_case("watchlist") {
        "watchlist".to_string()
    } else {
        "criteria".to_string()
    }
}

fn dedupe_stock_codes(codes: &[String]) -> Vec<String> {
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

fn normalize_stock_code(code: &str) -> String {
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

pub fn trend_with_mock(request: &TrendIndicatorRequest) -> CoreResult<TrendIndicatorResult> {
    trend_with_source(&MockDataSource, request)
}

pub fn trend_with_data(
    data: &CoreDataSet,
    request: &TrendIndicatorRequest,
) -> CoreResult<TrendIndicatorResult> {
    let source = StaticDataSource::new(data);
    trend_with_source(&source, request)
}

pub fn trend_with_source(
    source: &impl MarketDataSource,
    request: &TrendIndicatorRequest,
) -> CoreResult<TrendIndicatorResult> {
    let universe = source.stocks()?;
    let stock = universe
        .iter()
        .find(|stock| stock.code.as_str() == request.code.as_str())
        .cloned()
        .ok_or_else(|| CoreError::new(format!("未找到股票 {}", request.code)))?;
    let history = source.get_history(&stock.code, &request.start_date, &request.end_date)?;
    let bars = prepare_bars(&history, &stock)?;
    if bars.is_empty() {
        return Err(CoreError::new(format!("{} 没有可用历史行情", stock.code)));
    }
    let computed = compute_trend_bars(&bars);
    let signal = trend_signal_from_bar(&stock.code, computed.last().expect("non-empty trend bars"));
    let limit = clamp_series_limit(request.series_limit);
    let skip = computed.len().saturating_sub(limit);
    let series = computed
        .iter()
        .skip(skip)
        .map(trend_point_from_bar)
        .collect();
    Ok(TrendIndicatorResult {
        stock,
        signal,
        series,
    })
}

pub fn observe_with_mock(request: &StockObserveRequest) -> CoreResult<StockObservation> {
    observe_with_source(&MockDataSource, request)
}

pub fn observe_with_data(
    data: &CoreDataSet,
    request: &StockObserveRequest,
) -> CoreResult<StockObservation> {
    let source = StaticDataSource::new(data);
    observe_with_source(&source, request)
}

pub fn observe_with_source(
    source: &impl MarketDataSource,
    request: &StockObserveRequest,
) -> CoreResult<StockObservation> {
    let code = normalize_stock_code(&request.code);
    let universe = source.stocks()?;
    let stock = universe
        .iter()
        .find(|stock| normalize_stock_code(&stock.code) == code)
        .cloned()
        .ok_or_else(|| {
            CoreError::new(format!(
                "{}{}",
                "\u{672a}\u{627e}\u{5230}\u{80a1}\u{7968}\u{ff1a}", request.code
            ))
        })?;
    let mut notes = vec!["\u{6570}\u{636e}\u{6e90}\u{ff1a}Tauri/Rust \u{7edf}\u{4e00}\u{672c}\u{5730}\u{884c}\u{60c5}\u{7f13}\u{5b58}\u{3002}".to_string()];
    let financial_snapshot = source.get_financial_snapshot(&stock.code);
    let provided_capital_evidence = source.get_capital_evidence(&stock.code);
    let financial_indicators =
        build_observation_financial_indicators(&stock, financial_snapshot.as_ref());
    let trend_request = TrendIndicatorRequest {
        code: stock.code.clone(),
        start_date: request.start_date.clone(),
        end_date: request.end_date.clone(),
        series_limit: clamp_series_limit(request.series_limit),
    };
    let trend = match trend_with_source(source, &trend_request) {
        Ok(result) => Some(result),
        Err(error) => {
            notes.push(format!(
                "{}{}",
                "\u{8d8b}\u{52bf}\u{6307}\u{6807}\u{4e0d}\u{53ef}\u{7528}\u{ff1a}", error
            ));
            None
        }
    };
    if request.include_order_book {
        notes.push("基准为候选池买入持有等权曲线，用于衡量调仓与成本后的超额收益。".to_string());
    }
    let capital_evidence = build_observation_capital_evidence(
        &stock,
        &financial_indicators,
        trend.as_ref(),
        &request.end_date,
        provided_capital_evidence,
    );
    Ok(StockObservation {
        source: "tdx".to_string(),
        stock,
        financial_indicators: Some(financial_indicators),
        trend,
        capital_evidence: Some(capital_evidence),
        order_book: None,
        notes,
    })
}

mod observation;
pub use observation::*;

fn score_sentiment(score: f64) -> &'static str {
    if score >= 60.0 {
        "positive"
    } else if score <= 40.0 {
        "negative"
    } else {
        "neutral"
    }
}

fn capital_trade_date(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.len() >= 10
        && raw.as_bytes().get(4) == Some(&b'-')
        && raw.as_bytes().get(7) == Some(&b'-')
    {
        return Some(raw[..10].to_string());
    }
    let digits = raw
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.len() < 8 {
        return None;
    }
    NaiveDate::parse_from_str(&digits[..8], "%Y%m%d")
        .ok()
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn trend_screen_with_mock(request: &TrendScreenRequest) -> CoreResult<TrendScreenResult> {
    trend_screen_with_source(&MockDataSource, request)
}

pub fn trend_screen_with_data(
    data: &CoreDataSet,
    request: &TrendScreenRequest,
) -> CoreResult<TrendScreenResult> {
    let source = StaticDataSource::new(data);
    trend_screen_with_source(&source, request)
}

pub fn trend_screen_candidate_codes(
    universe: &[StockItem],
    request: &TrendScreenRequest,
) -> Vec<String> {
    screen_candidate_pool(universe, &request.criteria, request.limit)
        .into_iter()
        .map(|candidate| candidate.stock.code)
        .collect()
}

pub fn trend_screen_with_source(
    source: &impl MarketDataSource,
    request: &TrendScreenRequest,
) -> CoreResult<TrendScreenResult> {
    let universe = source.stocks()?;
    let candidate_pool = screen_candidate_pool(universe, &request.criteria, request.limit);
    let mut items = Vec::new();
    let mut skipped = 0_usize;
    let mut outdated = Vec::new();

    for candidate in &candidate_pool {
        let trend_request = TrendIndicatorRequest {
            code: candidate.stock.code.clone(),
            start_date: request.start_date.clone(),
            end_date: request.end_date.clone(),
            series_limit: 60,
        };
        let Ok(analysis) = trend_with_source(source, &trend_request) else {
            skipped += 1;
            continue;
        };
        if trend_history_behind_quote(&candidate.stock, &analysis.signal, &request.end_date) {
            outdated.push(format!(
                "{}：日线截至 {}，落后于 {} 的行情，需补齐日线后重新计算依据。",
                candidate.stock.code,
                analysis.signal.date,
                capital_trade_date(candidate.stock.quote_time.as_deref().unwrap_or_default())
                    .unwrap_or_default(),
            ));
            continue;
        }
        let layered = layered_trend_score(&candidate.stock, &analysis);
        if !trend_screen_candidate_allowed(&layered) {
            skipped += 1;
            continue;
        }
        let mut signal = analysis.signal.clone();
        signal.signal_type = layered.signal_type.clone();
        signal.risk_flags = layered.risk_flags.clone();
        signal.technical_score = Some(round6(layered.technical_score));
        signal.pattern_layer_score = Some(round6(layered.pattern_score));
        signal.quality_score = Some(round6(layered.quality_score));
        let mut reasons = candidate.reasons.clone();
        reasons.extend(
            signal
                .reasons
                .iter()
                .filter(|reason| {
                    !signal.risk_flags.contains(reason)
                        && reason.as_str() != "cyan_watch"
                        && !(reason.as_str() == "kdj_oversold" && signal.kdj_overbought)
                })
                .cloned(),
        );
        reasons.extend(layered.reason_tags.clone());
        reasons.sort();
        reasons.dedup();
        items.push(TrendStockSignal {
            stock: candidate.stock.clone(),
            base_score: round6(candidate.score),
            trend_score: round6(layered.trend_score),
            final_score: round6(layered.final_score),
            signal: signal.clone(),
            reasons,
            explanation: trend_screen_explanation(candidate, &signal, &layered),
        });
    }
    items.sort_by(|left, right| right.final_score.total_cmp(&left.final_score));
    items.truncate(request.limit.clamp(1, 100));
    let mut notes = trend_notes();
    notes.extend(deducted_profit_rule_notes(universe, &request.criteria));
    if !outdated.is_empty() {
        notes.push(format!(
            "{} 只候选的日线落后行情，未使用旧指标参与当前趋势排名。",
            outdated.len()
        ));
        notes.extend(outdated.into_iter().take(20));
    }
    if skipped > 0 {
        notes.push(format!(
            "Skipped {skipped} stocks without usable OHLC history."
        ));
    }
    Ok(TrendScreenResult {
        total: candidate_pool.len(),
        returned: items.len(),
        items,
        screen_style: "balanced_swing".to_string(),
        notes,
    })
}

fn trend_history_behind_quote(
    stock: &StockItem,
    signal: &TrendIndicatorSignal,
    end_date: &str,
) -> bool {
    let quote_date = stock.quote_time.as_deref().and_then(capital_trade_date);
    let Some(quote_date) = quote_date.and_then(|date| parse_date(&date).ok()) else {
        return false;
    };
    match (parse_date(&signal.date), parse_date(end_date)) {
        // A deliberately historical window can precede the current quote.
        (Ok(history_date), Ok(end_date)) => quote_date <= end_date && history_date < quote_date,
        _ => false,
    }
}

mod agent;
pub use agent::*;

fn mode_reply(mode: Option<&str>, text: &str) -> String {
    match mode.unwrap_or("research") {
        "quick" => text.to_string(),
        "expert" => format!(
            "{text}\n\n风险提示：以上为本地策略结果，需结合基本面与市场环境复核。\n下一步：可对候选股做观察或回测验证。"
        ),
        _ => format!("{text} 仅供选股研究，不构成投资建议。"),
    }
}

fn mobile_source_to_finding(source: &MobileStockSourceItem) -> MobileStockSkillFinding {
    let source_tier = normalize_source_tier(&source.source_tier);
    let evidence = first_non_empty(&[
        source.evidence.as_str(),
        source.summary.as_str(),
        source.title.as_str(),
    ]);
    let label = classify_mobile_source(&source_tier, &source.title, &source.summary, &evidence);
    let confidence = mobile_confidence(&source_tier, &label, !evidence.is_empty());

    MobileStockSkillFinding {
        label: label.clone(),
        title: truncate_chars(
            &first_non_empty(&[source.title.as_str(), source.summary.as_str(), "未命名信源"]),
            80,
        ),
        summary: mobile_finding_summary(&label, source),
        source_tier,
        source_name: first_non_empty(&[source.source_name.as_str(), "未标注来源"]),
        published_at: source.published_at.clone(),
        evidence: truncate_chars(&evidence, 220),
        confidence,
        risk_note: mobile_risk_note(&label),
    }
}

fn classify_mobile_source(source_tier: &str, title: &str, summary: &str, evidence: &str) -> String {
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

fn mobile_confidence(source_tier: &str, label: &str, has_evidence: bool) -> f64 {
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

fn mobile_finding_summary(label: &str, source: &MobileStockSourceItem) -> String {
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

fn mobile_risk_note(label: &str) -> String {
    let note = match label {
        "positive" => "需要用后续公告、财报和成交数据验证，不能据此作交易决定。",
        "negative" => "需要确认事项影响范围和持续性，不能单独作为交易依据。",
        "neutral" => "属于信息披露或背景材料，需结合财务和行情继续验证。",
        _ => "来源不足或未被官方披露确认，只能作为待验证线索。",
    };
    format!("{note} 仅供选股研究，不构成投资建议。")
}

fn mobile_overall_label(positive: usize, negative: usize, unverified: usize) -> String {
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

fn mobile_overview_summary(
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
fn sort_findings(findings: &mut [MobileStockSkillFinding]) {
    findings.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| right.published_at.cmp(&left.published_at))
    });
}

fn normalize_source_tier(value: &str) -> String {
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

fn first_non_empty(values: &[&str]) -> String {
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

fn truncate_chars(value: &str, max_chars: usize) -> String {
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

fn deducted_profit_rule_notes(universe: &[StockItem], criteria: &ScreenCriteria) -> Vec<String> {
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

fn has_deducted_profit_rule_metrics(stock: &StockItem, criteria: &ScreenCriteria) -> bool {
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

pub fn mock_stocks() -> Vec<StockItem> {
    vec![
        StockItem {
            code: "600519.SH".to_string(),
            name: "贵州茅台".to_string(),
            industry: "白酒".to_string(),
            is_st: false,
            price: 1700.0,
            pe: Some(32.1),
            pb: Some(10.5),
            roe: Some(0.32),
            market_cap_billion: Some(2200.0),
            dividend_yield: Some(0.02),
            deducted_net_profit_billion: Some(680.0),
            deducted_net_profit_margin: Some(48.0),
            deducted_net_profit_growth_rate: Some(12.0),
            ..StockItem::default()
        },
        StockItem {
            code: "000001.SZ".to_string(),
            name: "平安银行".to_string(),
            industry: "银行".to_string(),
            is_st: false,
            price: 12.3,
            pe: Some(5.8),
            pb: Some(0.6),
            roe: Some(0.12),
            market_cap_billion: Some(240.0),
            dividend_yield: Some(0.05),
            deducted_net_profit_billion: Some(450.0),
            deducted_net_profit_margin: Some(38.0),
            deducted_net_profit_growth_rate: Some(6.0),
            ..StockItem::default()
        },
        StockItem {
            code: "300750.SZ".to_string(),
            name: "宁德时代".to_string(),
            industry: "动力电池".to_string(),
            is_st: false,
            price: 195.0,
            pe: Some(25.5),
            pb: Some(6.5),
            roe: Some(0.18),
            market_cap_billion: Some(900.0),
            dividend_yield: Some(0.01),
            deducted_net_profit_billion: Some(380.0),
            deducted_net_profit_margin: Some(16.0),
            deducted_net_profit_growth_rate: Some(18.0),
            ..StockItem::default()
        },
        StockItem {
            code: "002594.SZ".to_string(),
            name: "比亚迪".to_string(),
            industry: "汽车".to_string(),
            is_st: false,
            price: 246.0,
            pe: Some(22.4),
            pb: Some(4.8),
            roe: Some(0.22),
            market_cap_billion: Some(720.0),
            dividend_yield: Some(0.006),
            deducted_net_profit_billion: Some(250.0),
            deducted_net_profit_margin: Some(12.0),
            deducted_net_profit_growth_rate: Some(15.0),
            ..StockItem::default()
        },
        StockItem {
            code: "002475.SZ".to_string(),
            name: "立讯精密".to_string(),
            industry: "电子制造".to_string(),
            is_st: false,
            price: 36.8,
            pe: Some(24.0),
            pb: Some(4.1),
            roe: Some(0.17),
            market_cap_billion: Some(260.0),
            dividend_yield: Some(0.008),
            deducted_net_profit_billion: Some(95.0),
            deducted_net_profit_margin: Some(11.0),
            deducted_net_profit_growth_rate: Some(11.0),
            ..StockItem::default()
        },
        StockItem {
            code: "600036.SH".to_string(),
            name: "招商银行".to_string(),
            industry: "银行".to_string(),
            is_st: false,
            price: 31.2,
            pe: Some(6.9),
            pb: Some(0.9),
            roe: Some(0.14),
            market_cap_billion: Some(900.0),
            dividend_yield: Some(0.04),
            deducted_net_profit_billion: Some(1300.0),
            deducted_net_profit_margin: Some(42.0),
            deducted_net_profit_growth_rate: Some(5.0),
            ..StockItem::default()
        },
        StockItem {
            code: "600000.SH".to_string(),
            name: "浦发银行".to_string(),
            industry: "银行".to_string(),
            is_st: false,
            price: 9.1,
            pe: Some(4.8),
            pb: Some(0.5),
            roe: Some(0.11),
            market_cap_billion: Some(170.0),
            dividend_yield: Some(0.06),
            deducted_net_profit_billion: Some(320.0),
            deducted_net_profit_margin: Some(36.0),
            deducted_net_profit_growth_rate: Some(4.0),
            ..StockItem::default()
        },
        StockItem {
            code: "601012.SH".to_string(),
            name: "隆基绿能".to_string(),
            industry: "光伏".to_string(),
            is_st: false,
            price: 18.6,
            pe: Some(14.2),
            pb: Some(1.8),
            roe: Some(0.13),
            market_cap_billion: Some(140.0),
            dividend_yield: Some(0.012),
            deducted_net_profit_billion: Some(90.0),
            deducted_net_profit_margin: Some(10.5),
            deducted_net_profit_growth_rate: Some(12.0),
            ..StockItem::default()
        },
        StockItem {
            code: "600309.SH".to_string(),
            name: "万华化学".to_string(),
            industry: "化工".to_string(),
            is_st: false,
            price: 78.4,
            pe: Some(15.6),
            pb: Some(2.6),
            roe: Some(0.19),
            market_cap_billion: Some(245.0),
            dividend_yield: Some(0.025),
            deducted_net_profit_billion: Some(160.0),
            deducted_net_profit_margin: Some(13.0),
            deducted_net_profit_growth_rate: Some(14.0),
            ..StockItem::default()
        },
        StockItem {
            code: "600887.SH".to_string(),
            name: "伊利股份".to_string(),
            industry: "食品饮料".to_string(),
            is_st: false,
            price: 28.5,
            pe: Some(18.2),
            pb: Some(3.1),
            roe: Some(0.16),
            market_cap_billion: Some(185.0),
            dividend_yield: Some(0.035),
            deducted_net_profit_billion: Some(100.0),
            deducted_net_profit_margin: Some(9.0),
            deducted_net_profit_growth_rate: Some(8.0),
            ..StockItem::default()
        },
    ]
}

pub fn mock_relations() -> Vec<StockRelation> {
    vec![
        StockRelation {
            source_code: "600036.SH".to_string(),
            target_code: "000001.SZ".to_string(),
            relation_type: "industry_peer".to_string(),
            weight: 0.75,
            description: Some("同受银行估值与信用周期影响。".to_string()),
        },
        StockRelation {
            source_code: "600036.SH".to_string(),
            target_code: "600000.SH".to_string(),
            relation_type: "industry_peer".to_string(),
            weight: 0.8,
            description: Some("A 股大型商业银行同业对比。".to_string()),
        },
        StockRelation {
            source_code: "000001.SZ".to_string(),
            target_code: "600000.SH".to_string(),
            relation_type: "industry_peer".to_string(),
            weight: 0.7,
            description: Some("区域银行与信用周期同业信号。".to_string()),
        },
        StockRelation {
            source_code: "300750.SZ".to_string(),
            target_code: "002594.SZ".to_string(),
            relation_type: "supply_chain".to_string(),
            weight: 0.65,
            description: Some("动力电池与新能源汽车需求链联动。".to_string()),
        },
        StockRelation {
            source_code: "300750.SZ".to_string(),
            target_code: "601012.SH".to_string(),
            relation_type: "thematic".to_string(),
            weight: 0.45,
            description: Some("清洁能源资本开支周期相关。".to_string()),
        },
        StockRelation {
            source_code: "002475.SZ".to_string(),
            target_code: "002594.SZ".to_string(),
            relation_type: "manufacturing_chain".to_string(),
            weight: 0.35,
            description: Some("先进制造与出口需求链联动。".to_string()),
        },
        StockRelation {
            source_code: "600309.SH".to_string(),
            target_code: "300750.SZ".to_string(),
            relation_type: "upstream_material".to_string(),
            weight: 0.3,
            description: Some("化工材料与新能源供应链上游相关。".to_string()),
        },
    ]
}

fn finite_positive(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

fn meets_min_rate(value: Option<f64>, minimum: f64, unit: RateUnit, inclusive: bool) -> bool {
    let Some(value) = value.and_then(|v| RateRatio::from_unit(v, unit).ok()) else {
        return false;
    };
    let Ok(minimum) = RateRatio::from_unit(minimum, unit) else {
        return false;
    };
    if inclusive {
        value.as_ratio() >= minimum.as_ratio()
    } else {
        value.as_ratio() > minimum.as_ratio()
    }
}

fn matches_stock(stock: &StockItem, criteria: &ScreenCriteria) -> Option<Vec<String>> {
    let mut reasons = Vec::new();
    if !stock.price.is_finite() || stock.price <= 0.0 {
        return None;
    }
    if !criteria.include_st && stock.is_st {
        return None;
    }

    if let Some(industry) = criteria.industry.as_ref() {
        let stock_value = stock.industry.trim();
        let selected_value = industry.trim();
        if !selected_value.is_empty()
            && (stock_value.is_empty() || !industry_matches(stock_value, selected_value))
        {
            return None;
        }
    }

    if let Some(market_scope) = criteria.market_scope.as_ref() {
        let selected_value = market_scope.trim();
        if !selected_value.is_empty() && market_scope_for_stock(stock) != selected_value {
            return None;
        }
    }

    if let Some(min_roe) = criteria.min_roe {
        if !meets_min_rate(stock.roe, min_roe, RateUnit::Ratio, true) {
            return None;
        }
        reasons.push("roe_ok".to_string());
    }

    if let Some(max_pe) = criteria.max_pe {
        if !max_pe.is_finite()
            || max_pe <= 0.0
            || finite_positive(stock.pe)
                .map(|pe| pe > max_pe)
                .unwrap_or(true)
        {
            return None;
        }
        reasons.push("pe_ok".to_string());
    }

    if let Some(max_pb) = criteria.max_pb {
        if !max_pb.is_finite()
            || max_pb <= 0.0
            || finite_positive(stock.pb)
                .map(|pb| pb > max_pb)
                .unwrap_or(true)
        {
            return None;
        }
        reasons.push("pb_ok".to_string());
    }

    if let Some(min_market_cap) = criteria.min_market_cap_billion {
        if !min_market_cap.is_finite()
            || min_market_cap < 0.0
            || finite_positive(stock.market_cap_billion)
                .map(|market_cap| market_cap < min_market_cap)
                .unwrap_or(true)
        {
            return None;
        }
        reasons.push("mcap_ok".to_string());
    }

    if let Some(min_profit) = criteria.min_deducted_net_profit_billion {
        if !min_profit.is_finite()
            || stock
                .deducted_net_profit_billion
                .filter(|profit| profit.is_finite())
                .map(|profit| profit <= min_profit)
                .unwrap_or(true)
        {
            return None;
        }
        reasons.push("deducted_net_profit_ok".to_string());
    }

    if let Some(min_margin) = criteria.min_deducted_net_profit_margin {
        if !meets_min_rate(
            stock.deducted_net_profit_margin,
            min_margin,
            RateUnit::PercentagePoints,
            false,
        ) {
            return None;
        }
        reasons.push("deducted_net_profit_margin_ok".to_string());
    }

    if let Some(min_growth) = criteria.min_deducted_net_profit_growth_rate {
        if !meets_min_rate(
            stock.deducted_net_profit_growth_rate,
            min_growth,
            RateUnit::PercentagePoints,
            false,
        ) {
            return None;
        }
        reasons.push("deducted_net_profit_growth_rate_ok".to_string());
    }

    Some(reasons)
}

fn industry_matches(stock_value: &str, selected_value: &str) -> bool {
    if stock_value == selected_value {
        return true;
    }

    if let Some(aliases) = legacy_industry_aliases(selected_value) {
        return aliases
            .iter()
            .any(|alias| stock_value == *alias || stock_value.contains(alias));
    }

    false
}

fn legacy_industry_aliases(selected_value: &str) -> Option<&'static [&'static str]> {
    match selected_value {
        "银行" => Some(&["银行Ⅱ", "银行服务"]),
        "证券" => Some(&["证券Ⅱ"]),
        "保险" => Some(&["保险Ⅱ"]),
        "中药" => Some(&["中药Ⅱ"]),
        "医药生物" => Some(&[
            "化学制药",
            "中药Ⅱ",
            "生物制品",
            "医疗器械",
            "医疗服务",
            "医药商业",
        ]),
        "食品饮料" => Some(&[
            "白酒Ⅱ",
            "非白酒",
            "食品加工",
            "调味发酵品Ⅱ",
            "饮料乳品",
            "休闲食品",
        ]),
        "白酒" => Some(&["白酒Ⅱ"]),
        "家用电器" => Some(&[
            "白色家电",
            "黑色家电",
            "厨卫电器",
            "小家电",
            "照明设备Ⅱ",
            "家电零部件Ⅱ",
            "其他家电Ⅱ",
        ]),
        "汽车整车" => Some(&["乘用车", "商用车", "摩托车及其他"]),
        "零部件" => Some(&["汽车零部件"]),
        "电力设备" => Some(&[
            "电池",
            "电机Ⅱ",
            "电网设备",
            "光伏设备",
            "风电设备",
            "其他电源设备Ⅱ",
        ]),
        "光伏" => Some(&["光伏设备"]),
        "风电" => Some(&["风电设备"]),
        "有色金属" => Some(&["工业金属", "贵金属", "能源金属", "小金属", "金属新材料"]),
        "钢铁" => Some(&["普钢", "特钢Ⅱ", "冶钢原料"]),
        "煤炭" => Some(&["煤炭开采", "焦炭Ⅱ"]),
        "石油" => Some(&["炼化及贸易", "油服工程", "油气开采Ⅱ"]),
        "化工" => Some(&[
            "化学纤维",
            "化学原料",
            "化学制品",
            "农化制品",
            "塑料",
            "橡胶",
            "非金属材料Ⅱ",
        ]),
        "建材" => Some(&["水泥", "玻璃玻纤", "装修建材"]),
        "建筑装饰" => Some(&[
            "房屋建设Ⅱ",
            "基础建设",
            "专业工程",
            "工程咨询服务Ⅱ",
            "装修装饰Ⅱ",
        ]),
        "计算机" => Some(&["计算机设备", "软件开发", "IT服务Ⅱ"]),
        "软件" => Some(&["软件开发"]),
        "通信" => Some(&["通信服务", "通信设备"]),
        "传媒" => Some(&[
            "出版",
            "电视广播Ⅱ",
            "广告营销",
            "数字媒体",
            "影视院线",
            "游戏Ⅱ",
        ]),
        "国防军工" => Some(&[
            "地面兵装Ⅱ",
            "航海装备Ⅱ",
            "航空装备Ⅱ",
            "航天装备Ⅱ",
            "军工电子Ⅱ",
        ]),
        "航空航天" => Some(&["航空装备Ⅱ", "航天装备Ⅱ"]),
        "机械设备" => Some(&[
            "工程机械",
            "轨交设备Ⅱ",
            "通用设备",
            "专用设备",
            "自动化设备",
        ]),
        "环保" => Some(&["环保设备Ⅱ", "环境治理"]),
        "农业" => Some(&[
            "动物保健Ⅱ",
            "林业Ⅱ",
            "农产品加工",
            "农业综合Ⅱ",
            "饲料",
            "养殖业",
            "渔业",
            "种植业",
        ]),
        "纺织服装" => Some(&["纺织制造", "服装家纺", "饰品"]),
        "商贸零售" => Some(&["贸易Ⅱ", "一般零售", "互联网电商", "专业连锁Ⅱ"]),
        "社会服务" => Some(&[
            "教育",
            "酒店餐饮",
            "旅游及景区",
            "旅游零售Ⅱ",
            "体育Ⅱ",
            "专业服务",
        ]),
        _ => None,
    }
}

fn market_scope_for_stock(stock: &StockItem) -> &'static str {
    let code = stock.code.trim().to_ascii_uppercase();
    let digits = code.split('.').next().unwrap_or("");
    if code.ends_with(".BJ")
        || digits.starts_with('8')
        || digits.starts_with('4')
        || digits.starts_with('9')
    {
        "北交所"
    } else if digits.starts_with("688") || digits.starts_with("689") {
        "科创板"
    } else if digits.starts_with("300") || digits.starts_with("301") {
        "创业板"
    } else if code.ends_with(".SH") || digits.starts_with('6') {
        "沪市A股"
    } else {
        "深市A股"
    }
}

const SCREEN_GROUP_LIMIT: usize = 10;
const POTENTIAL_SCORE_THRESHOLD: f64 = 10.0;
const SCREEN_SCORE_SCALE: f64 = 20.0;
const THEME_PROMOTION_ORDER: [&str; 7] = [
    "materials",
    "ai_chain",
    "semiconductor_wafer",
    "tech",
    "energy",
    "medical",
    "game",
];
const THEME_FILL_ORDER: [&str; 7] = [
    "ai_chain",
    "semiconductor_wafer",
    "materials",
    "tech",
    "energy",
    "medical",
    "game",
];
const THEME_RULES: [(&str, &str, f64, &[&str]); 7] = [
    (
        "materials",
        "\u{65b0}\u{6750}\u{6599}",
        0.96,
        &[
            "\u{6c1f}\u{5316}\u{5de5}",
            "\u{6c1f}\u{6750}\u{6599}",
            "\u{9502}\u{7535}\u{6750}\u{6599}",
            "\u{7535}\u{89e3}\u{6db2}",
            "\u{516d}\u{6c1f}\u{78f7}\u{9178}\u{9502}",
            "\u{65b0}\u{80fd}\u{6750}",
            "\u{65b0}\u{6750}\u{6599}",
            "\u{56fa}\u{6001}\u{7535}\u{6c60}",
            "\u{78c1}\u{6750}",
        ],
    ),
    (
        "semiconductor_wafer",
        "\u{534a}\u{5bfc}\u{4f53}\u{6676}\u{5706}",
        0.9,
        &[
            "\u{534a}\u{5bfc}\u{4f53}\u{6676}\u{5706}",
            "\u{6676}\u{5706}",
            "\u{6676}\u{5706}\u{4ee3}\u{5de5}",
            "\u{6676}\u{5706}\u{5236}\u{9020}",
            "\u{6676}\u{5706}\u{5382}",
            "\u{7845}\u{6676}\u{5706}",
            "\u{7845}\u{7247}",
            "\u{5916}\u{5ef6}\u{7247}",
            "\u{5916}\u{5ef6}\u{7845}\u{7247}",
            "\u{534a}\u{5bfc}\u{4f53}\u{886c}\u{5e95}",
            "\u{886c}\u{5e95}",
            "\u{78b3}\u{5316}\u{7845}\u{886c}\u{5e95}",
            "sic\u{886c}\u{5e95}",
            "\u{629b}\u{5149}\u{7247}",
            "8\u{82f1}\u{5bf8}",
            "12\u{82f1}\u{5bf8}",
        ],
    ),
    (
        "ai_chain",
        "AI\u{7b97}\u{529b}\u{4e0e}\u{82af}\u{7247}",
        0.95,
        &[
            "\u{534a}\u{5bfc}\u{4f53}",
            "\u{82af}\u{7247}",
            "\u{7b97}\u{529b}",
            "\u{4eba}\u{5de5}\u{667a}\u{80fd}",
            "ai",
            "\u{5149}\u{6a21}\u{5757}",
            "cpo",
            "\u{670d}\u{52a1}\u{5668}",
            "\u{6db2}\u{51b7}",
            "gpu",
            "hbm",
            "\u{5b58}\u{50a8}",
            "\u{6570}\u{636e}\u{4e2d}\u{5fc3}",
            "\u{4e91}\u{8ba1}\u{7b97}",
            "\u{5927}\u{6a21}\u{578b}",
            "aigc",
            "\u{8fb9}\u{7f18}\u{8ba1}\u{7b97}",
            "pcb",
            "\u{5c01}\u{88c5}",
            "\u{5c01}\u{6d4b}",
            "eda",
            "soc",
        ],
    ),
    (
        "tech",
        "\u{79d1}\u{6280}\u{5236}\u{9020}",
        0.84,
        &[
            "\u{673a}\u{5668}\u{4eba}",
            "\u{8f6f}\u{4ef6}",
            "\u{901a}\u{4fe1}",
            "\u{79d1}\u{6280}",
            "\u{7535}\u{5b50}",
            "\u{81ea}\u{52a8}\u{5316}",
            "\u{9ad8}\u{7aef}\u{5236}\u{9020}",
            "\u{667a}\u{80fd}\u{5236}\u{9020}",
        ],
    ),
    (
        "energy",
        "\u{65b0}\u{80fd}\u{6e90}",
        0.82,
        &[
            "\u{65b0}\u{80fd}\u{6e90}",
            "\u{7535}\u{6c60}",
            "\u{50a8}\u{80fd}",
            "\u{5149}\u{4f0f}",
            "\u{7535}\u{529b}",
            "\u{80fd}\u{6e90}",
            "\u{6cb9}\u{6c14}",
            "\u{7164}\u{70ad}",
            "\u{98ce}\u{7535}",
            "\u{5145}\u{7535}\u{6869}",
        ],
    ),
    (
        "medical",
        "\u{533b}\u{836f}\u{533b}\u{7597}",
        0.8,
        &[
            "\u{533b}\u{836f}",
            "\u{533b}\u{7597}",
            "\u{751f}\u{7269}\u{5236}\u{54c1}",
            "\u{521b}\u{65b0}\u{836f}",
            "\u{4e2d}\u{836f}",
            "\u{5316}\u{5b66}\u{5236}\u{836f}",
            "\u{533b}\u{7597}\u{5668}\u{68b0}",
            "cro",
            "cxo",
            "\u{75ab}\u{82d7}",
            "\u{533b}\u{7597}\u{670d}\u{52a1}",
            "\u{4eff}\u{5236}\u{836f}",
        ],
    ),
    (
        "game",
        "\u{6e38}\u{620f}\u{4f20}\u{5a92}",
        0.78,
        &[
            "\u{6e38}\u{620f}",
            "\u{7f51}\u{7edc}\u{6e38}\u{620f}",
            "\u{624b}\u{6e38}",
            "\u{7535}\u{7ade}",
            "\u{4e91}\u{6e38}\u{620f}",
            "\u{4e92}\u{52a8}\u{5a31}\u{4e50}",
            "\u{6587}\u{5316}\u{4f20}\u{5a92}",
            "\u{4f20}\u{5a92}",
        ],
    ),
];
const CONCEPT_GROUP_RULES: [(&str, &[&str]); 21] = [
    (
        "半导体设计",
        &[
            "半导体设计",
            "芯片设计",
            "集成电路设计",
            "ic设计",
            "soc",
            "mcu",
            "模拟芯片",
            "功率芯片",
            "射频芯片",
            "传感器芯片",
            "eda",
        ],
    ),
    (
        "半导体设备",
        &[
            "半导体设备",
            "光刻机",
            "刻蚀",
            "薄膜沉积",
            "离子注入",
            "量测设备",
            "检测设备",
            "清洗设备",
            "涂胶显影",
            "测试设备",
            "探针台",
            "分选机",
        ],
    ),
    (
        "半导体材料",
        &[
            "半导体材料",
            "光刻胶",
            "电子特气",
            "湿电子化学品",
            "靶材",
            "抛光液",
            "抛光垫",
            "cmp",
            "硅片",
            "衬底",
            "碳化硅",
            "sic",
            "氮化镓",
            "gan",
        ],
    ),
    (
        "半导体晶圆",
        &[
            "半导体晶圆",
            "晶圆",
            "晶圆代工",
            "晶圆制造",
            "晶圆厂",
            "硅晶圆",
            "外延片",
            "外延硅片",
            "半导体衬底",
            "碳化硅衬底",
            "sic衬底",
            "抛光片",
            "8英寸",
            "12英寸",
        ],
    ),
    (
        "半导体封测",
        &[
            "封测",
            "封装测试",
            "半导体封装",
            "芯片封装",
            "先进封装",
            "测试服务",
            "晶圆测试",
            "chiplet",
            "2.5d",
            "3d封装",
            "cowos",
        ],
    ),
    (
        "存储芯片",
        &[
            "存储芯片",
            "存储器",
            "dram",
            "nand",
            "nor flash",
            "hbm",
            "固态硬盘",
            "ssd",
            "闪存",
            "内存",
        ],
    ),
    (
        "AI算力与芯片",
        &[
            "算力",
            "人工智能",
            "ai",
            "光模块",
            "cpo",
            "服务器",
            "液冷",
            "gpu",
            "数据中心",
            "云计算",
            "大模型",
            "aigc",
            "边缘计算",
            "pcb",
        ],
    ),
    ("半导体", &["半导体", "集成电路", "芯片制造", "芯片产业"]),
    (
        "新材料",
        &[
            "氟化工",
            "氟材料",
            "锂电材料",
            "电解液",
            "六氟磷酸锂",
            "新能材",
            "新材料",
            "固态电池",
            "磁材",
        ],
    ),
    (
        "新能源与储能",
        &[
            "新能源",
            "电池",
            "储能",
            "光伏",
            "电力",
            "能源",
            "风电",
            "充电桩",
        ],
    ),
    (
        "游戏传媒",
        &[
            "游戏",
            "网络游戏",
            "手游",
            "电竞",
            "云游戏",
            "互动娱乐",
            "传媒",
            "广告营销",
        ],
    ),
    (
        "机器人与高端制造",
        &[
            "机器人",
            "工业母机",
            "自动化",
            "高端制造",
            "智能制造",
            "机械设备",
        ],
    ),
    (
        "消费零售",
        &[
            "食品",
            "饮料",
            "白酒",
            "休闲食品",
            "一般零售",
            "商贸零售",
            "家电",
            "旅游",
            "酒店",
            "餐饮",
        ],
    ),
    (
        "医药医疗",
        &[
            "医药",
            "医疗",
            "生物制品",
            "创新药",
            "中药",
            "化学制药",
            "医疗器械",
            "cro",
        ],
    ),
    (
        "金融地产",
        &["银行", "证券", "保险", "房地产", "地产", "物业"],
    ),
    (
        "基建建筑",
        &[
            "建筑",
            "房屋建设",
            "工程建设",
            "基础建设",
            "水泥",
            "铁路",
            "公路",
            "装修装饰",
        ],
    ),
    (
        "周期资源",
        &[
            "煤炭", "钢铁", "普钢", "有色", "金属", "化工", "石油", "油气", "矿业",
        ],
    ),
    (
        "汽车产业链",
        &[
            "汽车",
            "整车",
            "零部件",
            "轮胎",
            "智能驾驶",
            "无人驾驶",
            "汽车服务",
        ],
    ),
    (
        "军工航天",
        &["军工", "航天", "航空", "卫星", "船舶", "无人机", "国防"],
    ),
    (
        "交运物流",
        &[
            "物流",
            "航运",
            "港口",
            "机场",
            "航空运输",
            "铁路运输",
            "快递",
        ],
    ),
    ("公用环保", &["环保", "水务", "燃气", "供热", "公用事业"]),
];
const COLD_SECTOR_KEYWORDS: [&str; 9] = [
    "\u{94f6}\u{884c}",
    "\u{57fa}\u{5efa}",
    "\u{5efa}\u{7b51}",
    "\u{5efa}\u{7b51}\u{88c5}\u{9970}",
    "\u{5de5}\u{7a0b}\u{5efa}\u{8bbe}",
    "\u{57fa}\u{7840}\u{5efa}\u{8bbe}",
    "\u{6c34}\u{6ce5}",
    "\u{94c1}\u{8def}",
    "\u{516c}\u{8def}",
];

mod scoring;
pub use scoring::*;

fn concept_group_for_stock(stock: &StockItem) -> String {
    let text = stock_text(stock);
    for (label, keywords) in CONCEPT_GROUP_RULES {
        if contains_any(&text, keywords) {
            return label.to_string();
        }
    }
    "\u{5176}\u{4ed6}\u{6982}\u{5ff5}".to_string()
}

fn concept_sector_group_for_stock(stock: &StockItem) -> String {
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

fn concept_rank(concept: &str) -> usize {
    CONCEPT_GROUP_RULES
        .iter()
        .position(|(label, _)| *label == concept)
        .unwrap_or(CONCEPT_GROUP_RULES.len())
}

fn board_group_for_stock(stock: &StockItem) -> &'static str {
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

fn board_rank(board: &str) -> usize {
    match board {
        "\u{79d1}\u{521b}\u{677f}" => 0,
        "\u{521b}\u{4e1a}\u{677f}" => 1,
        "\u{5317}\u{4ea4}\u{6240}" => 2,
        "\u{6caa}\u{4e3b}\u{677f}" => 3,
        "\u{6df1}\u{4e3b}\u{677f}" => 4,
        _ => 99,
    }
}

fn theme_match_for_stock(stock: &StockItem) -> Option<(&'static str, &'static str, f64)> {
    theme_match_for_text(&stock_text(stock))
}

fn theme_match_for_text(text: &str) -> Option<(&'static str, &'static str, f64)> {
    for (key, label, score, keywords) in THEME_RULES {
        if contains_any(text, keywords) {
            return Some((key, label, score));
        }
    }
    None
}

fn stock_text(stock: &StockItem) -> String {
    format!("{} {}", stock.name, stock.industry).to_lowercase()
}

fn is_cold_sector(industry: &str) -> bool {
    let normalized = industry.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }
    COLD_SECTOR_KEYWORDS
        .iter()
        .any(|keyword| normalized.contains(keyword))
}

fn should_promote_hot_sectors(criteria: &ScreenCriteria) -> bool {
    criteria.industry.as_deref().unwrap_or("").trim().is_empty()
        && criteria.sort_by.trim().eq_ignore_ascii_case("score")
        && !criteria.sort_dir.trim().eq_ignore_ascii_case("asc")
        && normalized_score_profile(&criteria.score_profile) != "quality"
}

fn promote_hot_sector_items(
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

fn apply_industry_relative_scores(screened: &mut [ScreenedStock], score_profile: &str) {
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

fn industry_percentiles(screened: &[ScreenedStock], factor_key: &str) -> HashMap<String, f64> {
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

fn diversify_by_industry(screened: &[ScreenedStock], limit: usize) -> Vec<ScreenedStock> {
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
fn sort_screened(screened: &mut [ScreenedStock], criteria: &ScreenCriteria) {
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

fn should_diversify_by_industry(criteria: &ScreenCriteria) -> bool {
    criteria.industry.as_deref().unwrap_or("").trim().is_empty()
        && criteria.sort_by.trim().eq_ignore_ascii_case("score")
        && !criteria.sort_dir.trim().eq_ignore_ascii_case("asc")
        && normalized_score_profile(&criteria.score_profile) != "quality"
}

fn primary_screen_items(
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

fn screen_result_groups(screened: &[ScreenedStock]) -> Vec<ScreenResultGroup> {
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

fn hot_group_items(screened: &[ScreenedStock], limit: usize) -> Vec<ScreenedStock> {
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

fn potential_group_items(
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

fn first_by_theme(
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

fn append_selected(
    selected: &mut Vec<ScreenedStock>,
    used_codes: &mut HashSet<String>,
    item: ScreenedStock,
) {
    used_codes.insert(item.stock.code.clone());
    selected.push(item);
}

fn sort_value(item: &ScreenedStock, sort_by: &str) -> Option<f64> {
    let value = match sort_by {
        "price" => Some(item.stock.price),
        "pe" => item.stock.pe,
        "pb" => item.stock.pb,
        "roe" => item.stock.roe.and_then(ratio_percentage_points),
        "market_cap" | "market_cap_billion" => item.stock.market_cap_billion,
        "change_pct" | "pct" => item.stock.change_pct.and_then(ratio_percentage_points),
        _ => Some(item.score),
    };
    value.filter(|value| value.is_finite())
}

fn screen_candidate_pool(
    universe: &[StockItem],
    criteria: &ScreenCriteria,
    requested_limit: usize,
) -> Vec<ScreenedStock> {
    let pool_size = 200.min((requested_limit * 5).max(criteria.limit).max(50));
    let mut expanded_criteria = criteria.clone();
    expanded_criteria.limit = pool_size;
    screen_stocks(universe, &expanded_criteria).items
}

fn assign_weights(signals: &mut [GraphStockSignal]) {
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

fn prepare_bars(history: &[HistoryBar], stock: &StockItem) -> CoreResult<Vec<PreparedBar>> {
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

fn compute_trend_bars(bars: &[PreparedBar]) -> Vec<ComputedTrendBar> {
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

fn primary_state(close: &[f64], up: bool) -> Vec<bool> {
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

fn alternating_states(primary: &[bool], close: &[f64], starts_with_le_ge: bool) -> Vec<Vec<bool>> {
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

fn or_states(states: &[Vec<bool>]) -> Vec<bool> {
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

fn ema(values: &[f64], span: usize) -> Vec<f64> {
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

fn ma(values: &[f64], window: usize) -> Vec<f64> {
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

fn rolling_sum(values: &[f64], window: usize) -> Vec<f64> {
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

fn rolling_mean_partial(values: &[f64], window: usize) -> Vec<f64> {
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

fn rolling_min(values: &[f64], window: usize) -> Vec<f64> {
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

fn rolling_max(values: &[f64], window: usize) -> Vec<f64> {
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

fn dma(values: &[f64], alpha: &[f64]) -> Vec<f64> {
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

fn tdx_sma(values: &[f64], window: f64, weight: f64) -> Vec<f64> {
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

fn kdj(close: &[f64], high: &[f64], low: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
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

fn quant_scores(close: &[f64], high: &[f64], low: &[f64], volume: &[f64]) -> Vec<i32> {
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
struct CapitalBehaviorBar {
    accumulation_index: f64,
    accumulation_strength: f64,
    swing_opportunity: f64,
    rebound_signal: f64,
    trend_heat: f64,
    volume_price_heat: f64,
    anomaly_heat: f64,
    popularity_heat: f64,
}

fn capital_behavior_metrics(
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

fn star_line(close: &[f64], open: &[f64], high: &[f64], low: &[f64]) -> Vec<f64> {
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

fn trend_signal_from_bar(code: &str, bar: &ComputedTrendBar) -> TrendIndicatorSignal {
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

fn trend_point_from_bar(bar: &ComputedTrendBar) -> TrendIndicatorPoint {
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

fn trend_reasons(bar: &ComputedTrendBar) -> Vec<String> {
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

fn pattern_score(bar: &ComputedTrendBar) -> i32 {
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

fn pattern_signals(bar: &ComputedTrendBar) -> Vec<String> {
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

fn trend_status(bar: &ComputedTrendBar) -> String {
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
struct TrendLayerScores {
    technical_score: f64,
    pattern_score: f64,
    quality_score: f64,
    trend_score: f64,
    final_score: f64,
    signal_type: String,
    risk_flags: Vec<String>,
    reason_tags: Vec<String>,
}

fn layered_trend_score(stock: &StockItem, trend: &TrendIndicatorResult) -> TrendLayerScores {
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

fn trend_screen_candidate_allowed(layered: &TrendLayerScores) -> bool {
    !layered
        .risk_flags
        .iter()
        .any(|flag| flag == "bearish_long_ma_stack")
}

fn technical_layer_score(signal: &TrendIndicatorSignal, bars: &[TrendIndicatorPoint]) -> f64 {
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

fn pattern_layer_score(signal: &TrendIndicatorSignal, bars: &[TrendIndicatorPoint]) -> f64 {
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

fn quality_layer_score(stock: &StockItem) -> f64 {
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

fn classify_trend_signal(
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

fn layered_reason_tags(
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

fn trend_layer_risk_flags(
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

fn trend_risk_flags_from_bar(bar: &ComputedTrendBar) -> Vec<String> {
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

fn closes_from_points(bars: &[TrendIndicatorPoint]) -> Vec<f64> {
    bars.iter().map(|bar| bar.close).collect()
}

fn volumes_from_points(bars: &[TrendIndicatorPoint]) -> Vec<f64> {
    bars.iter()
        .map(|bar| bar.volume.unwrap_or(0.0).max(0.0))
        .collect()
}

fn trailing_mean(values: &[f64], window: usize) -> f64 {
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

fn trailing_max(values: &[f64], window: usize) -> f64 {
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

fn trailing_min(values: &[f64], window: usize) -> f64 {
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

fn pct_change(current: f64, previous: f64) -> Option<f64> {
    if current.is_finite() && previous.is_finite() && previous.abs() > f64::EPSILON {
        Some((current / previous - 1.0) * 100.0)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MacdSnapshot {
    dif: f64,
    dea: f64,
    hist: f64,
    prev_dif: f64,
    prev_dea: f64,
    prev_hist: f64,
}

fn macd_snapshot(closes: &[f64]) -> MacdSnapshot {
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

fn high_upper_shadow_ratio(bars: &[TrendIndicatorPoint]) -> Option<f64> {
    let bar = bars.last()?;
    let high = bar.high?;
    let open = bar.open.unwrap_or(bar.close);
    let low = bar.low?;
    if !high.is_finite() || !open.is_finite() || !low.is_finite() || high <= low {
        return None;
    }
    Some(((high - open.max(bar.close)) / (high - low)).clamp(0.0, 1.0))
}
fn trend_screen_explanation(
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

fn trend_screen_signal_label(signal_type: &str) -> &str {
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

fn trend_indicator_value(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "—".into())
}

fn pattern_basis_text(signal: &TrendIndicatorSignal) -> Option<String> {
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

fn trend_risk_checks(signal: &TrendIndicatorSignal) -> Vec<String> {
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

fn trend_notes() -> Vec<String> {
    vec![
        "吸筹分析根据日线价量推断，暂未纳入真实筹码分布、龙虎榜席位和主力资金流。".to_string(),
        "量化评分满分为90分，筹码分布相关指标尚未纳入。".to_string(),
        "长期均线根据量能动态平滑，用于趋势参考。".to_string(),
    ]
}

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

fn history_bars_in_range(
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

fn history_points_from_bars(history: &[HistoryBar]) -> CoreResult<Vec<HistoryPoint>> {
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

fn mock_history(
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

fn parse_date(input: &str) -> CoreResult<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y%m%d")
        .or_else(|_| NaiveDate::parse_from_str(input, "%Y-%m-%d"))
        .map_err(|_| CoreError::new(format!("Invalid date: {input}")))
}

fn backtest_metrics(
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

fn heuristic_criteria(message: &str) -> ScreenCriteria {
    ScreenCriteria {
        max_pe: extract_number_after(message, &["PE", "pe", "市盈率"]),
        max_pb: extract_number_after(message, &["PB", "pb", "市净率"]),
        min_roe: extract_percent_after(message, &["ROE", "roe", "净资产收益率"]),
        industry: extract_industry(message),
        ..ScreenCriteria::default()
    }
}

fn extract_number_after(message: &str, labels: &[&str]) -> Option<f64> {
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

fn extract_percent_after(message: &str, labels: &[&str]) -> Option<f64> {
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

fn extract_industry(message: &str) -> Option<String> {
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

fn extract_codes(message: &str) -> Vec<String> {
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

fn extract_date(message: &str, default: &str, first: bool) -> String {
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
fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| text.contains(&needle.to_lowercase()))
}

fn score_contribution(
    key: &str,
    label: &str,
    value: f64,
    contribution: f64,
    strong_threshold: f64,
    positive_threshold: f64,
) -> ScoreContribution {
    let tone = if value >= strong_threshold {
        "strong"
    } else if value >= positive_threshold {
        "positive"
    } else if value <= 0.0 {
        "weak"
    } else {
        "neutral"
    };
    ScoreContribution {
        key: key.to_string(),
        label: label.to_string(),
        value: Some(round6(value)),
        contribution: Some(round6(contribution)),
        tone: tone.to_string(),
    }
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn finite_round4(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(round4(value))
    } else {
        None
    }
}

mod ffi;

#[cfg(test)]
mod tests;
