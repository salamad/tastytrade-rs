//! Market data models for snapshot quotes.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::InstrumentType;

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
    #[serde(default)]
    pub volume: Option<i64>,
    /// Open interest (for options/futures)
    #[serde(default)]
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
    /// Current implied volatility index
    #[serde(default)]
    pub implied_volatility_index: Option<Decimal>,
    /// IV index 5-day change
    #[serde(default)]
    pub implied_volatility_index_5_day_change: Option<Decimal>,
    /// Historical volatility index
    #[serde(default)]
    pub historical_volatility_index: Option<Decimal>,
    /// HV percentile (0-100)
    #[serde(default)]
    pub historical_volatility_index_percentile: Option<Decimal>,
    /// IV/HV ratio
    #[serde(default)]
    pub iv_hv_ratio: Option<Decimal>,
    /// Market cap
    #[serde(default)]
    pub market_cap: Option<Decimal>,
    /// Earnings date
    #[serde(default)]
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
    #[serde(default)]
    pub shares_outstanding: Option<i64>,
    /// When the metrics were last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl MarketMetric {
    /// Check if IV rank indicates high implied volatility (above 50).
    pub fn is_high_iv_rank(&self) -> bool {
        self.implied_volatility_index_rank
            .map(|rank| rank > Decimal::from(50))
            .unwrap_or(false)
    }

    /// Check if IV is elevated compared to historical volatility.
    pub fn is_iv_elevated(&self) -> bool {
        self.iv_hv_ratio
            .map(|ratio| ratio > Decimal::from(1))
            .unwrap_or(false)
    }
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
            implied_volatility_index: Some(dec!(0.25)),
            implied_volatility_index_5_day_change: None,
            historical_volatility_index: Some(dec!(0.20)),
            historical_volatility_index_percentile: None,
            iv_hv_ratio: Some(dec!(1.25)),
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
    }
}
