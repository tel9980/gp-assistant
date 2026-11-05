mod adaptive_screen;
mod backtest;
mod industry_rank;
mod rates;
#[cfg(test)]
mod scoring_contract_tests;
mod scoring_parameters;
mod source_fingerprint;
mod volatility;
pub use backtest::*;
mod observation;
pub use observation::*;
mod agent;
pub use agent::*;
mod scoring;
pub use scoring::*;
mod sector;
pub use sector::*;
mod trend;
pub use trend::*;
mod dataset;
pub use dataset::*;
mod mobile;
pub use mobile::*;
mod screening;
pub use screening::*;
mod ffi;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    sync::OnceLock,
};

use chrono::{Datelike, Days, Local, NaiveDate, Weekday};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use adaptive_screen::{
    adaptive_candidate_codes, adaptive_scoring_features, adaptive_screen_stocks,
    adaptive_screen_stocks_with_parameters, AdaptiveFeatureRow, AdaptiveFeatureSet,
    AdaptiveMarketRegime, AdaptiveRecentExposure, AdaptiveScreenRequest, AdaptiveScreenResult,
    MIN_MARKET_BREADTH_COVERAGE,
};
pub(crate) use industry_rank::industry_rank_values;
use rates::{percentage_points, ratio_percentage_points};
pub use rates::{RateRatio, RateUnit};
pub use scoring_parameters::{
    adaptive_score_factor_vector, fixed_scoring_parameters, scoring_implementation_fingerprint,
    validate_scoring_parameters, ModeParameters, ScoringParameters, METRIC_CONTRACT_VERSION,
    SCORING_REVISION,
};
pub use source_fingerprint::source_text_fingerprint;
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
pub struct HistoryPoint {
    date: NaiveDate,
    close: f64,
}

#[derive(Clone, Debug)]
pub struct PreparedBar {
    date: NaiveDate,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    capital: f64,
}

#[derive(Clone, Debug)]
pub struct ComputedTrendBar {
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
