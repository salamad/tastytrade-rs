//! DXLink event types.
//!
//! These types represent real-time market data events received via the DXLink
//! WebSocket streaming protocol.

use rust_decimal::Decimal;
use serde::Deserialize;

use super::{DxEventTrait, EventType};

/// Quote event - current bid/ask prices and sizes.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Time of the last bid/ask update in nanoseconds
    pub time_nano_part: Option<i32>,
    /// Bid time in milliseconds
    pub bid_time: Option<i64>,
    /// Bid exchange code
    pub bid_exchange_code: Option<String>,
    /// Bid price
    pub bid_price: Option<Decimal>,
    /// Bid size
    pub bid_size: Option<Decimal>,
    /// Ask time in milliseconds
    pub ask_time: Option<i64>,
    /// Ask exchange code
    pub ask_exchange_code: Option<String>,
    /// Ask price
    pub ask_price: Option<Decimal>,
    /// Ask size
    pub ask_size: Option<Decimal>,
}

impl DxEventTrait for Quote {
    fn event_type() -> EventType {
        EventType::Quote
    }
}

/// Trade event - last trade information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Time in nanoseconds (nano part)
    pub time_nano_part: Option<i32>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Exchange code
    pub exchange_code: Option<String>,
    /// Last trade price
    pub price: Option<Decimal>,
    /// Change from previous close
    pub change: Option<Decimal>,
    /// Last trade size
    pub size: Option<Decimal>,
    /// Day volume
    pub day_volume: Option<Decimal>,
    /// Day turnover
    pub day_turnover: Option<Decimal>,
    /// Tick direction
    pub tick_direction: Option<String>,
    /// Extended trading hours flag
    pub extended_trading_hours: Option<bool>,
}

impl DxEventTrait for Trade {
    fn event_type() -> EventType {
        EventType::Trade
    }
}

/// Greeks event - option Greeks values.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Greeks {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Option price
    pub price: Option<Decimal>,
    /// Volatility
    pub volatility: Option<Decimal>,
    /// Delta
    pub delta: Option<Decimal>,
    /// Gamma
    pub gamma: Option<Decimal>,
    /// Theta
    pub theta: Option<Decimal>,
    /// Rho
    pub rho: Option<Decimal>,
    /// Vega
    pub vega: Option<Decimal>,
}

impl DxEventTrait for Greeks {
    fn event_type() -> EventType {
        EventType::Greeks
    }
}

/// Summary event - daily summary (OHLC) data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Day ID
    pub day_id: Option<i32>,
    /// Day open price
    pub day_open_price: Option<Decimal>,
    /// Day high price
    pub day_high_price: Option<Decimal>,
    /// Day low price
    pub day_low_price: Option<Decimal>,
    /// Day close price
    pub day_close_price: Option<Decimal>,
    /// Previous day ID
    pub prev_day_id: Option<i32>,
    /// Previous day close price
    pub prev_day_close_price: Option<Decimal>,
    /// Previous day volume
    pub prev_day_volume: Option<Decimal>,
    /// Open interest
    pub open_interest: Option<i64>,
}

impl DxEventTrait for Summary {
    fn event_type() -> EventType {
        EventType::Summary
    }
}

/// Profile event - security profile information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Description
    pub description: Option<String>,
    /// Short sale restriction
    pub short_sale_restriction: Option<String>,
    /// Trading status
    pub trading_status: Option<String>,
    /// Status reason
    pub status_reason: Option<String>,
    /// Halt start time
    pub halt_start_time: Option<i64>,
    /// Halt end time
    pub halt_end_time: Option<i64>,
    /// High limit price
    pub high_limit_price: Option<Decimal>,
    /// Low limit price
    pub low_limit_price: Option<Decimal>,
    /// High 52-week price
    pub high_52_week_price: Option<Decimal>,
    /// Low 52-week price
    pub low_52_week_price: Option<Decimal>,
    /// Beta
    pub beta: Option<Decimal>,
    /// Earnings per share
    pub earnings_per_share: Option<Decimal>,
    /// Dividend frequency
    pub dividend_frequency: Option<Decimal>,
    /// Ex-dividend amount
    pub ex_dividend_amount: Option<Decimal>,
    /// Ex-dividend day ID
    pub ex_dividend_day_id: Option<i32>,
    /// Shares outstanding
    pub shares: Option<i64>,
    /// Free float shares
    pub free_float: Option<i64>,
}

impl DxEventTrait for Profile {
    fn event_type() -> EventType {
        EventType::Profile
    }
}

/// Candle event - candlestick data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Count of trades in candle
    pub count: Option<i64>,
    /// Open price
    pub open: Option<Decimal>,
    /// High price
    pub high: Option<Decimal>,
    /// Low price
    pub low: Option<Decimal>,
    /// Close price
    pub close: Option<Decimal>,
    /// Volume
    pub volume: Option<Decimal>,
    /// Volume-weighted average price
    pub vwap: Option<Decimal>,
    /// Bid volume
    pub bid_volume: Option<Decimal>,
    /// Ask volume
    pub ask_volume: Option<Decimal>,
    /// Implied volatility
    pub imp_volatility: Option<Decimal>,
    /// Open interest
    pub open_interest: Option<i64>,
}

impl DxEventTrait for Candle {
    fn event_type() -> EventType {
        EventType::Candle
    }
}

/// Theoretical price event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TheoPrice {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Theoretical price
    pub price: Option<Decimal>,
    /// Underlying price
    pub underlying_price: Option<Decimal>,
    /// Delta
    pub delta: Option<Decimal>,
    /// Gamma
    pub gamma: Option<Decimal>,
    /// Dividend
    pub dividend: Option<Decimal>,
    /// Interest
    pub interest: Option<Decimal>,
}

impl DxEventTrait for TheoPrice {
    fn event_type() -> EventType {
        EventType::TheoPrice
    }
}

/// Time and Sales event - individual trade details.
///
/// This event provides detailed information about each individual trade,
/// including time, price, size, and exchange information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeAndSale {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Time in nanoseconds (nano part)
    pub time_nano_part: Option<i32>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Exchange code
    pub exchange_code: Option<String>,
    /// Trade price
    pub price: Option<Decimal>,
    /// Trade size
    pub size: Option<Decimal>,
    /// Bid price at time of trade
    pub bid_price: Option<Decimal>,
    /// Ask price at time of trade
    pub ask_price: Option<Decimal>,
    /// Exchange sale conditions
    pub exchange_sale_conditions: Option<String>,
    /// Trade through exempt flag
    pub trade_through_exempt: Option<bool>,
    /// Aggressor side (BUY, SELL, or UNDEFINED)
    pub aggressor_side: Option<String>,
    /// Spread leg flag
    pub spread_leg: Option<bool>,
    /// Extended trading hours flag
    pub extended_trading_hours: Option<bool>,
    /// Valid tick flag
    pub valid_tick: Option<bool>,
    /// Type of trade (REGULAR, CANCEL, CORRECTION, AS_OF, etc.)
    #[serde(rename = "type")]
    pub trade_type: Option<String>,
}

impl DxEventTrait for TimeAndSale {
    fn event_type() -> EventType {
        EventType::TimeAndSale
    }
}

/// Extended Trading Hours (ETH) Trade event.
///
/// Similar to Trade but specifically for extended hours trading sessions.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeETH {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Index
    pub index: Option<i64>,
    /// Time in milliseconds
    pub time: Option<i64>,
    /// Time in nanoseconds (nano part)
    pub time_nano_part: Option<i32>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Exchange code
    pub exchange_code: Option<String>,
    /// Last trade price
    pub price: Option<Decimal>,
    /// Change from previous close
    pub change: Option<Decimal>,
    /// Last trade size
    pub size: Option<Decimal>,
    /// Day volume
    pub day_volume: Option<Decimal>,
    /// Day turnover
    pub day_turnover: Option<Decimal>,
    /// Tick direction
    pub tick_direction: Option<String>,
    /// Extended trading hours flag (always true for TradeETH)
    pub extended_trading_hours: Option<bool>,
}

impl DxEventTrait for TradeETH {
    fn event_type() -> EventType {
        EventType::TradeETH
    }
}

/// Underlying event - information about the underlying security.
///
/// This provides data about the underlying instrument for derivatives.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Underlying {
    /// Event symbol
    pub event_symbol: String,
    /// Event time in milliseconds
    pub event_time: Option<i64>,
    /// Event flags
    pub event_flags: Option<i32>,
    /// Sequence number
    pub sequence: Option<i64>,
    /// Volatility
    pub volatility: Option<Decimal>,
    /// Front month volatility
    pub front_volatility: Option<Decimal>,
    /// Back month volatility
    pub back_volatility: Option<Decimal>,
    /// Call volume
    pub call_volume: Option<Decimal>,
    /// Put volume
    pub put_volume: Option<Decimal>,
    /// Put/call volume ratio
    pub put_call_ratio: Option<Decimal>,
}

impl DxEventTrait for Underlying {
    fn event_type() -> EventType {
        EventType::Underlying
    }
}
