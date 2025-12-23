//! Financial instrument models.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::{InstrumentType, OptionType};

/// Common fields for all instrument types.
pub trait Instrument {
    /// Get the trading symbol.
    fn symbol(&self) -> &str;
    /// Get the instrument type.
    fn instrument_type(&self) -> InstrumentType;
    /// Get the streamer symbol for market data subscriptions.
    fn streamer_symbol(&self) -> Option<&str>;
    /// Get the description.
    fn description(&self) -> Option<&str>;
}

/// Equity (stock/ETF) instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Equity {
    /// Trading symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default = "default_equity_type")]
    pub instrument_type: InstrumentType,
    /// Symbol for streaming market data
    #[serde(default)]
    pub streamer_symbol: Option<String>,
    /// Description/name of the security
    #[serde(default)]
    pub description: Option<String>,
    /// CUSIP number
    #[serde(default)]
    pub cusip: Option<String>,
    /// Whether this is actively traded
    #[serde(default)]
    pub is_active: bool,
    /// Whether this is an index
    #[serde(default)]
    pub is_index: bool,
    /// Whether this is an ETF
    #[serde(default)]
    pub is_etf: bool,
    /// Whether options are available
    #[serde(default)]
    pub is_options_tradeable: bool,
    /// Short description
    #[serde(default)]
    pub short_description: Option<String>,
    /// Listed market
    #[serde(default)]
    pub listed_market: Option<String>,
    /// Lot size
    #[serde(default)]
    pub lendability: Option<String>,
    /// Borrow rate
    #[serde(default)]
    pub borrow_rate: Option<Decimal>,
    /// Market time instrument collection
    #[serde(default)]
    pub market_time_instrument_collection: Option<String>,
    /// Whether this is closeable
    #[serde(default)]
    pub is_closing_only: bool,
    /// Whether this is fractional-share eligible
    #[serde(default)]
    pub is_fractional_quantity_eligible: bool,
}

fn default_equity_type() -> InstrumentType {
    InstrumentType::Equity
}

impl Instrument for Equity {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    fn streamer_symbol(&self) -> Option<&str> {
        self.streamer_symbol.as_deref()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Equity option instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EquityOption {
    /// Option symbol (OCC format)
    pub symbol: String,
    /// Instrument type
    #[serde(default = "default_option_type")]
    pub instrument_type: InstrumentType,
    /// Symbol for streaming
    #[serde(default)]
    pub streamer_symbol: Option<String>,
    /// Underlying symbol
    pub underlying_symbol: String,
    /// Strike price
    pub strike_price: Decimal,
    /// Expiration date
    pub expiration_date: NaiveDate,
    /// Expiration type
    #[serde(default)]
    pub expiration_type: Option<String>,
    /// Option type (call/put)
    pub option_type: OptionType,
    /// Contract size (usually 100)
    #[serde(default)]
    pub shares_per_contract: Option<i32>,
    /// Root symbol
    #[serde(default)]
    pub root_symbol: Option<String>,
    /// Whether this is actively traded
    #[serde(default)]
    pub is_active: bool,
    /// Option chain type
    #[serde(default)]
    pub option_chain_type: Option<String>,
    /// Days to expiration
    #[serde(default)]
    pub days_to_expiration: Option<i32>,
    /// Stop trading date
    #[serde(default)]
    pub stops_trading_at: Option<DateTime<Utc>>,
    /// Expires at
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// Settlement type
    #[serde(default)]
    pub settlement_type: Option<String>,
    /// Exercise style
    #[serde(default)]
    pub exercise_style: Option<String>,
}

fn default_option_type() -> InstrumentType {
    InstrumentType::EquityOption
}

impl Instrument for EquityOption {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    fn streamer_symbol(&self) -> Option<&str> {
        self.streamer_symbol.as_deref()
    }

    fn description(&self) -> Option<&str> {
        None
    }
}

impl EquityOption {
    /// Returns `true` if this is a call option.
    pub fn is_call(&self) -> bool {
        self.option_type.is_call()
    }

    /// Returns `true` if this is a put option.
    pub fn is_put(&self) -> bool {
        self.option_type.is_put()
    }

    /// Returns `true` if the option has expired.
    pub fn is_expired(&self) -> bool {
        self.expiration_date < chrono::Utc::now().date_naive()
    }

    /// Get the contract multiplier.
    pub fn multiplier(&self) -> i32 {
        self.shares_per_contract.unwrap_or(100)
    }
}

/// Futures contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Future {
    /// Futures symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default = "default_future_type")]
    pub instrument_type: InstrumentType,
    /// Symbol for streaming
    #[serde(default)]
    pub streamer_symbol: Option<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Product code
    #[serde(default)]
    pub product_code: Option<String>,
    /// Contract size
    #[serde(default)]
    pub contract_size: Option<Decimal>,
    /// Tick size
    #[serde(default)]
    pub tick_size: Option<Decimal>,
    /// Notional multiplier
    #[serde(default)]
    pub notional_multiplier: Option<Decimal>,
    /// Main fraction
    #[serde(default)]
    pub main_fraction: Option<Decimal>,
    /// Sub fraction
    #[serde(default)]
    pub sub_fraction: Option<Decimal>,
    /// Display factor
    #[serde(default)]
    pub display_factor: Option<Decimal>,
    /// Exchange code
    #[serde(default)]
    pub exchange_code: Option<String>,
    /// Product group
    #[serde(default)]
    pub product_group: Option<String>,
    /// Expiration date
    #[serde(default)]
    pub expiration_date: Option<NaiveDate>,
    /// First notice date
    #[serde(default)]
    pub first_notice_date: Option<NaiveDate>,
    /// Last trading date
    #[serde(default)]
    pub last_trade_date: Option<NaiveDate>,
    /// Whether this is actively traded
    #[serde(default)]
    pub is_active: bool,
    /// Stop trading date
    #[serde(default)]
    pub stops_trading_at: Option<DateTime<Utc>>,
    /// Expires at
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether this is closing only
    #[serde(default)]
    pub is_closing_only: bool,
}

fn default_future_type() -> InstrumentType {
    InstrumentType::Future
}

impl Instrument for Future {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    fn streamer_symbol(&self) -> Option<&str> {
        self.streamer_symbol.as_deref()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Futures option instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FutureOption {
    /// Option symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default = "default_future_option_type")]
    pub instrument_type: InstrumentType,
    /// Symbol for streaming
    #[serde(default)]
    pub streamer_symbol: Option<String>,
    /// Underlying futures symbol
    pub underlying_symbol: String,
    /// Strike price
    pub strike_price: Decimal,
    /// Expiration date
    pub expiration_date: NaiveDate,
    /// Option type (call/put)
    pub option_type: OptionType,
    /// Contract size
    #[serde(default)]
    pub contract_size: Option<Decimal>,
    /// Tick size
    #[serde(default)]
    pub tick_size: Option<Decimal>,
    /// Notional multiplier
    #[serde(default)]
    pub notional_multiplier: Option<Decimal>,
    /// Whether this is actively traded
    #[serde(default)]
    pub is_active: bool,
    /// Root symbol
    #[serde(default)]
    pub root_symbol: Option<String>,
    /// Exchange code
    #[serde(default)]
    pub exchange_code: Option<String>,
    /// Exercise style
    #[serde(default)]
    pub exercise_style: Option<String>,
    /// Settlement type
    #[serde(default)]
    pub settlement_type: Option<String>,
    /// Stop trading date
    #[serde(default)]
    pub stops_trading_at: Option<DateTime<Utc>>,
    /// Expires at
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

fn default_future_option_type() -> InstrumentType {
    InstrumentType::FutureOption
}

impl Instrument for FutureOption {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    fn streamer_symbol(&self) -> Option<&str> {
        self.streamer_symbol.as_deref()
    }

    fn description(&self) -> Option<&str> {
        None
    }
}

/// Cryptocurrency instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Cryptocurrency {
    /// Trading symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default = "default_crypto_type")]
    pub instrument_type: InstrumentType,
    /// Symbol for streaming
    #[serde(default)]
    pub streamer_symbol: Option<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Whether this is actively traded
    #[serde(default)]
    pub is_active: bool,
    /// Tick size
    #[serde(default)]
    pub tick_size: Option<Decimal>,
    /// Minimum order quantity
    #[serde(default)]
    pub minimum_quantity: Option<Decimal>,
    /// Quantity increment
    #[serde(default)]
    pub quantity_increment: Option<Decimal>,
    /// Price increment
    #[serde(default)]
    pub price_increment: Option<Decimal>,
    /// Destination venue
    #[serde(default)]
    pub destination_venue: Option<String>,
}

fn default_crypto_type() -> InstrumentType {
    InstrumentType::Cryptocurrency
}

impl Instrument for Cryptocurrency {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn instrument_type(&self) -> InstrumentType {
        self.instrument_type
    }

    fn streamer_symbol(&self) -> Option<&str> {
        self.streamer_symbol.as_deref()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Option chain for an underlying.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NestedOptionChain {
    /// Underlying symbol
    pub underlying_symbol: String,
    /// Root symbol
    #[serde(default)]
    pub root_symbol: Option<String>,
    /// Option chain type
    #[serde(default)]
    pub option_chain_type: Option<String>,
    /// Shares per contract
    #[serde(default)]
    pub shares_per_contract: Option<i32>,
    /// Expirations with strikes
    pub expirations: Vec<OptionExpiration>,
}

/// Option expiration with strikes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionExpiration {
    /// Expiration date
    pub expiration_date: NaiveDate,
    /// Expiration type
    #[serde(default)]
    pub expiration_type: Option<String>,
    /// Days to expiration
    #[serde(default)]
    pub days_to_expiration: Option<i32>,
    /// Settlement type
    #[serde(default)]
    pub settlement_type: Option<String>,
    /// Available strikes
    pub strikes: Vec<OptionStrike>,
}

/// Strike price with call and put symbols.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OptionStrike {
    /// Strike price
    pub strike_price: Decimal,
    /// Call option symbol
    #[serde(default)]
    pub call: Option<String>,
    /// Call streamer symbol
    #[serde(default)]
    pub call_streamer_symbol: Option<String>,
    /// Put option symbol
    #[serde(default)]
    pub put: Option<String>,
    /// Put streamer symbol
    #[serde(default)]
    pub put_streamer_symbol: Option<String>,
}

/// Symbol search result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SymbolSearchResult {
    /// Trading symbol
    #[serde(default)]
    pub symbol: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Instrument type
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_deserialize_equity() {
        let json = r#"{
            "symbol": "AAPL",
            "instrument-type": "Equity",
            "streamer-symbol": "AAPL",
            "description": "Apple Inc.",
            "is-active": true,
            "is-etf": false
        }"#;

        let equity: Equity = serde_json::from_str(json).unwrap();
        assert_eq!(equity.symbol, "AAPL");
        assert_eq!(equity.description, Some("Apple Inc.".to_string()));
        assert!(equity.is_active);
    }

    #[test]
    fn test_equity_option() {
        let option = EquityOption {
            symbol: "AAPL  240119C00150000".to_string(),
            instrument_type: InstrumentType::EquityOption,
            streamer_symbol: Some(".AAPL240119C150".to_string()),
            underlying_symbol: "AAPL".to_string(),
            strike_price: dec!(150),
            expiration_date: NaiveDate::from_ymd_opt(2024, 1, 19).unwrap(),
            expiration_type: None,
            option_type: OptionType::Call,
            shares_per_contract: Some(100),
            root_symbol: Some("AAPL".to_string()),
            is_active: true,
            option_chain_type: None,
            days_to_expiration: Some(30),
            stops_trading_at: None,
            expires_at: None,
            settlement_type: None,
            exercise_style: None,
        };

        assert!(option.is_call());
        assert!(!option.is_put());
        assert_eq!(option.multiplier(), 100);
    }
}
