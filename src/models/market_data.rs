//! Market data models for snapshot quotes.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};

use super::enums::InstrumentType;

/// Helper to deserialize i64 that may come as integers or strings (possibly with decimal points).
fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumValue {
        Int(i64),
        Float(f64),
        String(String),
        Null,
    }

    match NumValue::deserialize(deserializer)? {
        NumValue::Int(i) => Ok(Some(i)),
        NumValue::Float(f) => Ok(Some(f as i64)),
        NumValue::String(s) if s.is_empty() => Ok(None),
        NumValue::String(s) => {
            // Handle strings like "21521802.0"
            s.parse::<f64>()
                .map(|f| Some(f as i64))
                .map_err(D::Error::custom)
        }
        NumValue::Null => Ok(None),
    }
}

/// Market data snapshot for an instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarketData {
    /// Trading symbol
    pub symbol: String,
    /// Instrument type
    pub instrument_type: InstrumentType,
    /// Best bid price
    #[serde(default)]
    pub bid: Option<Decimal>,
    /// Best bid size
    #[serde(default)]
    pub bid_size: Option<Decimal>,
    /// Best ask price
    #[serde(default)]
    pub ask: Option<Decimal>,
    /// Best ask size
    #[serde(default)]
    pub ask_size: Option<Decimal>,
    /// Last trade price
    #[serde(default)]
    pub last: Option<Decimal>,
    /// Last trade size
    #[serde(default)]
    pub last_size: Option<Decimal>,
    /// Mark (midpoint) price
    #[serde(default)]
    pub mark: Option<Decimal>,
    /// Previous day's close
    #[serde(default)]
    pub prev_close: Option<Decimal>,
    /// Today's close (if available)
    #[serde(default)]
    pub close: Option<Decimal>,
    /// Today's high
    #[serde(default)]
    pub high: Option<Decimal>,
    /// Today's low
    #[serde(default)]
    pub low: Option<Decimal>,
    /// Today's open
    #[serde(default)]
    pub open: Option<Decimal>,
    /// Net change from previous close
    #[serde(default)]
    pub net_change: Option<Decimal>,
    /// Percent change from previous close
    #[serde(default)]
    pub percent_change: Option<Decimal>,
    /// Today's trading volume
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub volume: Option<i64>,
    /// Open interest (for options/futures)
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub open_interest: Option<i64>,
    /// Implied volatility (for options)
    #[serde(default)]
    pub implied_volatility: Option<Decimal>,
    /// Historical volatility
    #[serde(default)]
    pub historical_volatility: Option<Decimal>,
    /// 52-week high
    #[serde(default)]
    pub week_52_high: Option<Decimal>,
    /// 52-week low
    #[serde(default)]
    pub week_52_low: Option<Decimal>,
    /// When the data was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl MarketData {
    /// Calculate the bid-ask spread.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.ask, self.bid) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Calculate the spread as a percentage of the mid price.
    pub fn spread_percent(&self) -> Option<Decimal> {
        match (self.spread(), self.mark) {
            (Some(spread), Some(mark)) if mark > Decimal::ZERO => {
                Some((spread / mark) * Decimal::from(100))
            }
            _ => None,
        }
    }

    /// Returns `true` if quote data is available.
    pub fn has_quote(&self) -> bool {
        self.bid.is_some() && self.ask.is_some()
    }
}

/// Greeks for an option.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionGreeks {
    /// Option symbol
    pub symbol: String,
    /// Delta - rate of change of option price with respect to underlying
    #[serde(default)]
    pub delta: Option<Decimal>,
    /// Gamma - rate of change of delta
    #[serde(default)]
    pub gamma: Option<Decimal>,
    /// Theta - time decay per day
    #[serde(default)]
    pub theta: Option<Decimal>,
    /// Vega - sensitivity to implied volatility
    #[serde(default)]
    pub vega: Option<Decimal>,
    /// Rho - sensitivity to interest rates
    #[serde(default)]
    pub rho: Option<Decimal>,
    /// Implied volatility
    #[serde(default)]
    pub implied_volatility: Option<Decimal>,
    /// Theoretical price
    #[serde(default)]
    pub theo_price: Option<Decimal>,
    /// When the greeks were calculated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Detailed quote with Greeks from /market-data/by-type endpoint.
///
/// This model includes all market data fields plus option Greeks when
/// querying for option contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DetailedQuote {
    /// Trading symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default)]
    pub instrument_type: Option<String>,
    /// When the data was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// Best bid price
    #[serde(default)]
    pub bid: Option<Decimal>,
    /// Best bid size
    #[serde(default)]
    pub bid_size: Option<Decimal>,
    /// Best ask price
    #[serde(default)]
    pub ask: Option<Decimal>,
    /// Best ask size
    #[serde(default)]
    pub ask_size: Option<Decimal>,
    /// Midpoint price
    #[serde(default)]
    pub mid: Option<Decimal>,
    /// Mark price
    #[serde(default)]
    pub mark: Option<Decimal>,
    /// Last trade price
    #[serde(default)]
    pub last: Option<Decimal>,
    /// Last market price
    #[serde(default)]
    pub last_mkt: Option<Decimal>,
    /// Today's open price
    #[serde(default)]
    pub open: Option<Decimal>,
    /// Today's high price
    #[serde(default)]
    pub day_high_price: Option<Decimal>,
    /// Today's low price
    #[serde(default)]
    pub day_low_price: Option<Decimal>,
    /// Close price type
    #[serde(default)]
    pub close_price_type: Option<String>,
    /// Previous close price
    #[serde(default)]
    pub prev_close: Option<Decimal>,
    /// Previous close price type
    #[serde(default)]
    pub prev_close_price_type: Option<String>,
    /// Summary date
    #[serde(default)]
    pub summary_date: Option<NaiveDate>,
    /// Previous close date
    #[serde(default)]
    pub prev_close_date: Option<NaiveDate>,
    /// Whether trading is halted
    #[serde(default)]
    pub is_trading_halted: bool,
    /// Halt start time (epoch or -1 if not halted)
    #[serde(default)]
    pub halt_start_time: Option<i64>,
    /// Halt end time (epoch or -1 if not halted)
    #[serde(default)]
    pub halt_end_time: Option<i64>,
    /// Today's trading volume
    #[serde(default)]
    pub volume: Option<Decimal>,
    /// Implied volatility (for options)
    #[serde(default)]
    pub volatility: Option<Decimal>,
    /// Delta - rate of change of option price with respect to underlying
    #[serde(default)]
    pub delta: Option<Decimal>,
    /// Gamma - rate of change of delta
    #[serde(default)]
    pub gamma: Option<Decimal>,
    /// Theta - time decay per day
    #[serde(default)]
    pub theta: Option<Decimal>,
    /// Rho - sensitivity to interest rates
    #[serde(default)]
    pub rho: Option<Decimal>,
    /// Vega - sensitivity to implied volatility
    #[serde(default)]
    pub vega: Option<Decimal>,
    /// Theoretical price
    #[serde(default)]
    pub theo_price: Option<Decimal>,
    /// DXFeed mark price
    #[serde(default)]
    pub dx_mark: Option<Decimal>,
    /// Tick size
    #[serde(default)]
    pub tick_size: Option<Decimal>,
    /// Open interest
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub open_interest: Option<i64>,
}

impl DetailedQuote {
    /// Check if this quote has Greeks (i.e., is an option).
    pub fn has_greeks(&self) -> bool {
        self.delta.is_some() || self.gamma.is_some() || self.theta.is_some()
    }

    /// Calculate the bid-ask spread.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.ask, self.bid) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }
}

/// Implied volatility for a specific option expiration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionExpirationImpliedVolatility {
    /// Expiration date (as string, format varies)
    #[serde(default)]
    pub expiration_date: Option<String>,
    /// Settlement type (e.g., "AM", "PM")
    #[serde(default)]
    pub settlement_type: Option<String>,
    /// Option chain type (e.g., "Standard", "Weekly")
    #[serde(default)]
    pub option_chain_type: Option<String>,
    /// Implied volatility for this expiration
    #[serde(default)]
    pub implied_volatility: Option<Decimal>,
}

/// Market metrics for an underlying.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarketMetric {
    /// Trading symbol
    pub symbol: String,
    /// Implied volatility rank (0-100)
    #[serde(default)]
    pub implied_volatility_index_rank: Option<Decimal>,
    /// Implied volatility percentile (0-100)
    #[serde(default)]
    pub implied_volatility_index_percentile: Option<Decimal>,
    /// Implied volatility percentile (alternate field name)
    #[serde(default)]
    pub implied_volatility_percentile: Option<Decimal>,
    /// Current implied volatility index
    #[serde(default)]
    pub implied_volatility_index: Option<Decimal>,
    /// IV index 5-day change
    #[serde(default)]
    pub implied_volatility_index_5_day_change: Option<Decimal>,
    /// Implied volatility rank (alternate field name)
    #[serde(default)]
    pub implied_volatility_rank: Option<Decimal>,
    /// Historical volatility index
    #[serde(default)]
    pub historical_volatility_index: Option<Decimal>,
    /// HV percentile (0-100)
    #[serde(default)]
    pub historical_volatility_index_percentile: Option<Decimal>,
    /// IV/HV ratio
    #[serde(default)]
    pub iv_hv_ratio: Option<Decimal>,
    /// Liquidity value
    #[serde(default)]
    pub liquidity: Option<Decimal>,
    /// Liquidity rank (0-100)
    #[serde(default)]
    pub liquidity_rank: Option<Decimal>,
    /// Liquidity rating (0-5 stars)
    #[serde(default)]
    pub liquidity_rating: Option<Decimal>,
    /// Option expiration implied volatilities
    #[serde(default)]
    pub option_expiration_implied_volatilities: Vec<OptionExpirationImpliedVolatility>,
    /// Market cap
    #[serde(default)]
    pub market_cap: Option<Decimal>,
    /// Expected earnings report date from TastyTrade API
    /// Note: The API field is "expected-report-date", not "earnings-date"
    #[serde(default, rename = "expected-report-date")]
    pub earnings_date: Option<String>,
    /// Days until earnings
    #[serde(default)]
    pub days_until_earnings: Option<i32>,
    /// Expected move based on implied volatility
    #[serde(default)]
    pub expected_move: Option<Decimal>,
    /// Expected move percentage
    #[serde(default)]
    pub expected_move_percent: Option<Decimal>,
    /// Beta
    #[serde(default)]
    pub beta: Option<Decimal>,
    /// Correlation to SPY
    #[serde(default)]
    pub corr_spy_3_month: Option<Decimal>,
    /// Dividend yield
    #[serde(default)]
    pub dividend_yield: Option<Decimal>,
    /// Annual dividend
    #[serde(default)]
    pub annual_dividend: Option<Decimal>,
    /// Ex-dividend date
    #[serde(default)]
    pub ex_dividend_date: Option<String>,
    /// Shares outstanding
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    pub shares_outstanding: Option<i64>,
    /// When the metrics were last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl MarketMetric {
    /// Check if IV rank indicates high implied volatility (above 50).
    pub fn is_high_iv_rank(&self) -> bool {
        self.implied_volatility_index_rank
            .or(self.implied_volatility_rank)
            .map(|rank| rank > Decimal::from(50))
            .unwrap_or(false)
    }

    /// Check if IV is elevated compared to historical volatility.
    pub fn is_iv_elevated(&self) -> bool {
        self.iv_hv_ratio
            .map(|ratio| ratio > Decimal::from(1))
            .unwrap_or(false)
    }

    /// Get the effective IV rank (from either field name).
    pub fn effective_iv_rank(&self) -> Option<Decimal> {
        self.implied_volatility_index_rank.or(self.implied_volatility_rank)
    }

    /// Get the effective IV percentile (from either field name).
    pub fn effective_iv_percentile(&self) -> Option<Decimal> {
        self.implied_volatility_index_percentile.or(self.implied_volatility_percentile)
    }

    /// Check if this symbol has good liquidity (rating >= 3).
    pub fn has_good_liquidity(&self) -> bool {
        self.liquidity_rating
            .map(|rating| rating >= Decimal::from(3))
            .unwrap_or(false)
    }

    /// Get IV by expiration date string (e.g., "2025-01-17").
    pub fn iv_for_expiration(&self, date: &str) -> Option<Decimal> {
        self.option_expiration_implied_volatilities
            .iter()
            .find(|iv| iv.expiration_date.as_ref().map(|d| d.contains(date)).unwrap_or(false))
            .and_then(|iv| iv.implied_volatility)
    }
}

/// Current equity market session information.
///
/// Contains information about market hours, state, and next/previous sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquitySession {
    /// Session close time
    #[serde(default)]
    pub close_at: Option<DateTime<Utc>>,
    /// Extended hours close time
    #[serde(default)]
    pub close_at_ext: Option<DateTime<Utc>>,
    /// Instrument collection (e.g., "Equity")
    #[serde(default)]
    pub instrument_collection: Option<String>,
    /// Market open time
    #[serde(default)]
    pub open_at: Option<DateTime<Utc>>,
    /// Session start time (pre-market)
    #[serde(default)]
    pub start_at: Option<DateTime<Utc>>,
    /// Current market state (e.g., "Open", "Closed", "Pre-Market")
    #[serde(default)]
    pub state: Option<String>,
    /// Next trading session
    #[serde(default)]
    pub next_session: Option<SessionInfo>,
    /// Previous trading session
    #[serde(default)]
    pub previous_session: Option<SessionInfo>,
}

impl EquitySession {
    /// Check if the market is currently open.
    pub fn is_open(&self) -> bool {
        self.state.as_ref().map(|s| s == "Open").unwrap_or(false)
    }

    /// Check if we're in pre-market hours.
    pub fn is_pre_market(&self) -> bool {
        self.state.as_ref().map(|s| s.contains("Pre")).unwrap_or(false)
    }

    /// Check if we're in after-hours trading.
    pub fn is_after_hours(&self) -> bool {
        self.state.as_ref().map(|s| s.contains("After") || s.contains("Extended")).unwrap_or(false)
    }
}

/// Information about a specific trading session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SessionInfo {
    /// Session close time
    #[serde(default)]
    pub close_at: Option<DateTime<Utc>>,
    /// Extended hours close time
    #[serde(default)]
    pub close_at_ext: Option<DateTime<Utc>>,
    /// Instrument collection
    #[serde(default)]
    pub instrument_collection: Option<String>,
    /// Market open time
    #[serde(default)]
    pub open_at: Option<DateTime<Utc>>,
    /// Session date
    #[serde(default)]
    pub session_date: Option<NaiveDate>,
    /// Session start time
    #[serde(default)]
    pub start_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_market_data_spread() {
        let data = MarketData {
            symbol: "AAPL".to_string(),
            instrument_type: InstrumentType::Equity,
            bid: Some(dec!(149.95)),
            bid_size: Some(dec!(100)),
            ask: Some(dec!(150.05)),
            ask_size: Some(dec!(100)),
            last: Some(dec!(150.00)),
            last_size: Some(dec!(50)),
            mark: Some(dec!(150.00)),
            prev_close: Some(dec!(148.00)),
            close: None,
            high: Some(dec!(151.00)),
            low: Some(dec!(148.50)),
            open: Some(dec!(149.00)),
            net_change: Some(dec!(2.00)),
            percent_change: Some(dec!(1.35)),
            volume: Some(50000000),
            open_interest: None,
            implied_volatility: None,
            historical_volatility: None,
            week_52_high: Some(dec!(180.00)),
            week_52_low: Some(dec!(120.00)),
            updated_at: None,
        };

        assert_eq!(data.spread(), Some(dec!(0.10)));
        assert!(data.has_quote());
    }

    #[test]
    fn test_market_metric_iv_rank() {
        let metric = MarketMetric {
            symbol: "AAPL".to_string(),
            implied_volatility_index_rank: Some(dec!(75)),
            implied_volatility_index_percentile: Some(dec!(80)),
            implied_volatility_percentile: None,
            implied_volatility_index: Some(dec!(0.25)),
            implied_volatility_index_5_day_change: None,
            implied_volatility_rank: None,
            historical_volatility_index: Some(dec!(0.20)),
            historical_volatility_index_percentile: None,
            iv_hv_ratio: Some(dec!(1.25)),
            liquidity: None,
            liquidity_rank: None,
            liquidity_rating: Some(dec!(4)),
            option_expiration_implied_volatilities: vec![],
            market_cap: None,
            earnings_date: None,
            days_until_earnings: None,
            expected_move: None,
            expected_move_percent: None,
            beta: Some(dec!(1.2)),
            corr_spy_3_month: None,
            dividend_yield: None,
            annual_dividend: None,
            ex_dividend_date: None,
            shares_outstanding: None,
            updated_at: None,
        };

        assert!(metric.is_high_iv_rank());
        assert!(metric.is_iv_elevated());
        assert!(metric.has_good_liquidity());
        assert_eq!(metric.effective_iv_rank(), Some(dec!(75)));
    }
}
