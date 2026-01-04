//! COMPACT format parsing for DXLink events.
//!
//! DXLink uses COMPACT format where event data is sent as positional arrays
//! rather than JSON objects with named fields. This module provides parsing
//! logic to convert these arrays into typed event structs.
//!
//! # COMPACT Format Structure
//!
//! ```text
//! {"type":"FEED_DATA","channel":1,"data":["Quote",["Quote","SPY",450.25,450.30,100,200]]}
//! ```
//!
//! The data array structure is:
//! - `data[0]`: Event type string ("Quote", "Trade", etc.)
//! - `data[1]`: Array of event values in positional order

use rust_decimal::Decimal;
use std::str::FromStr;

use super::events::*;
use super::EventType;

/// Field definitions for each event type.
/// These match the `acceptEventFields` sent in FEED_SETUP.
pub mod fields {
    /// Quote event fields in COMPACT format order.
    pub const QUOTE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "bidPrice",
        "askPrice",
        "bidSize",
        "askSize",
    ];

    /// Trade event fields in COMPACT format order.
    pub const TRADE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "price",
        "dayVolume",
        "size",
    ];

    /// Greeks event fields in COMPACT format order.
    pub const GREEKS_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "volatility",
        "delta",
        "gamma",
        "theta",
        "rho",
        "vega",
    ];

    /// Summary event fields in COMPACT format order.
    pub const SUMMARY_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "openInterest",
        "dayOpenPrice",
        "dayHighPrice",
        "dayLowPrice",
        "prevDayClosePrice",
    ];

    /// Profile event fields in COMPACT format order.
    pub const PROFILE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "description",
        "shortSaleRestriction",
        "tradingStatus",
        "statusReason",
        "haltStartTime",
        "haltEndTime",
        "highLimitPrice",
        "lowLimitPrice",
        "high52WeekPrice",
        "low52WeekPrice",
    ];

    /// Candle event fields in COMPACT format order.
    pub const CANDLE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "eventFlags",
        "index",
        "time",
        "sequence",
        "count",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "vwap",
        "bidVolume",
        "askVolume",
        "impVolatility",
        "openInterest",
    ];

    /// TheoPrice event fields in COMPACT format order.
    pub const THEO_PRICE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "eventFlags",
        "index",
        "time",
        "sequence",
        "price",
        "underlyingPrice",
        "delta",
        "gamma",
        "dividend",
        "interest",
    ];

    /// TimeAndSale event fields in COMPACT format order.
    pub const TIME_AND_SALE_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "eventFlags",
        "index",
        "time",
        "timeNanoPart",
        "sequence",
        "exchangeCode",
        "price",
        "size",
        "bidPrice",
        "askPrice",
        "exchangeSaleConditions",
        "aggressorSide",
        "spreadLeg",
        "extendedTradingHours",
        "validTick",
        "type",
    ];

    /// TradeETH event fields (same structure as Trade).
    pub const TRADE_ETH_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "price",
        "dayVolume",
        "size",
    ];

    /// Underlying event fields in COMPACT format order.
    pub const UNDERLYING_FIELDS: &[&str] = &[
        "eventType",
        "eventSymbol",
        "eventFlags",
        "sequence",
        "volatility",
        "frontVolatility",
        "backVolatility",
        "callVolume",
        "putVolume",
        "putCallRatio",
    ];
}

/// Helper to extract a string from a JSON value at a given index.
fn get_string(arr: &[serde_json::Value], idx: usize) -> Option<String> {
    arr.get(idx).and_then(|v| {
        if v.is_null() {
            None
        } else {
            v.as_str().map(|s| s.to_string())
        }
    })
}

/// Helper to extract a Decimal from a JSON value at a given index.
///
/// Handles various JSON number formats including floats, integers, and strings.
/// Rounds to avoid floating-point precision artifacts when converting from f64.
fn get_decimal(arr: &[serde_json::Value], idx: usize) -> Option<Decimal> {
    arr.get(idx).and_then(|v| {
        if v.is_null() {
            return None;
        }
        // Handle "NaN" strings from DXLink
        if let Some(s) = v.as_str() {
            if s == "NaN" || s == "Infinity" || s == "-Infinity" {
                return None;
            }
            return Decimal::from_str(s).ok();
        }
        // Handle integer values first (exact conversion)
        if let Some(n) = v.as_i64() {
            return Some(Decimal::from(n));
        }
        // Handle numeric values (may need rounding to avoid float artifacts)
        if let Some(n) = v.as_f64() {
            if n.is_nan() || n.is_infinite() {
                return None;
            }
            // Convert via string to avoid float precision issues
            // This preserves the precision the JSON parser saw
            let s = format!("{}", n);
            return Decimal::from_str(&s).ok();
        }
        None
    })
}

/// Helper to extract an i64 from a JSON value at a given index.
fn get_i64(arr: &[serde_json::Value], idx: usize) -> Option<i64> {
    arr.get(idx).and_then(|v| {
        if v.is_null() {
            return None;
        }
        if let Some(n) = v.as_i64() {
            return Some(n);
        }
        if let Some(n) = v.as_f64() {
            if n.is_nan() || n.is_infinite() {
                return None;
            }
            return Some(n as i64);
        }
        None
    })
}

/// Helper to extract an i32 from a JSON value at a given index.
fn get_i32(arr: &[serde_json::Value], idx: usize) -> Option<i32> {
    get_i64(arr, idx).map(|n| n as i32)
}

/// Helper to extract a bool from a JSON value at a given index.
fn get_bool(arr: &[serde_json::Value], idx: usize) -> Option<bool> {
    arr.get(idx).and_then(|v| {
        if v.is_null() {
            None
        } else {
            v.as_bool()
        }
    })
}

/// Parse a Quote event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, bidPrice, askPrice, bidSize, askSize
pub fn parse_quote(arr: &[serde_json::Value]) -> Option<Quote> {
    // Minimum required: eventType + eventSymbol
    if arr.len() < 2 {
        return None;
    }

    Some(Quote {
        event_symbol: get_string(arr, 1)?,
        event_time: None, // Not in standard COMPACT fields
        sequence: None,
        time_nano_part: None,
        bid_time: None,
        bid_exchange_code: None,
        bid_price: get_decimal(arr, 2),
        ask_price: get_decimal(arr, 3),
        bid_size: get_decimal(arr, 4),
        ask_size: get_decimal(arr, 5),
        ask_time: None,
        ask_exchange_code: None,
    })
}

/// Parse a Trade event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, price, dayVolume, size
pub fn parse_trade(arr: &[serde_json::Value]) -> Option<Trade> {
    if arr.len() < 2 {
        return None;
    }

    Some(Trade {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: None,
        index: None,
        time: None,
        time_nano_part: None,
        sequence: None,
        exchange_code: None,
        price: get_decimal(arr, 2),
        change: None,
        size: get_decimal(arr, 4),
        day_volume: get_decimal(arr, 3),
        day_turnover: None,
        tick_direction: None,
        extended_trading_hours: None,
    })
}

/// Parse a Greeks event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, volatility, delta, gamma, theta, rho, vega
pub fn parse_greeks(arr: &[serde_json::Value]) -> Option<Greeks> {
    if arr.len() < 2 {
        return None;
    }

    Some(Greeks {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: None,
        index: None,
        time: None,
        sequence: None,
        price: None,
        volatility: get_decimal(arr, 2),
        delta: get_decimal(arr, 3),
        gamma: get_decimal(arr, 4),
        theta: get_decimal(arr, 5),
        rho: get_decimal(arr, 6),
        vega: get_decimal(arr, 7),
    })
}

/// Parse a Summary event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, openInterest, dayOpenPrice, dayHighPrice, dayLowPrice, prevDayClosePrice
pub fn parse_summary(arr: &[serde_json::Value]) -> Option<Summary> {
    if arr.len() < 2 {
        return None;
    }

    Some(Summary {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: None,
        day_id: None,
        open_interest: get_i64(arr, 2),
        day_open_price: get_decimal(arr, 3),
        day_high_price: get_decimal(arr, 4),
        day_low_price: get_decimal(arr, 5),
        day_close_price: None,
        prev_day_id: None,
        prev_day_close_price: get_decimal(arr, 6),
        prev_day_volume: None,
    })
}

/// Parse a Profile event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, description, shortSaleRestriction, tradingStatus,
/// statusReason, haltStartTime, haltEndTime, highLimitPrice, lowLimitPrice, high52WeekPrice, low52WeekPrice
pub fn parse_profile(arr: &[serde_json::Value]) -> Option<Profile> {
    if arr.len() < 2 {
        return None;
    }

    Some(Profile {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        description: get_string(arr, 2),
        short_sale_restriction: get_string(arr, 3),
        trading_status: get_string(arr, 4),
        status_reason: get_string(arr, 5),
        halt_start_time: get_i64(arr, 6),
        halt_end_time: get_i64(arr, 7),
        high_limit_price: get_decimal(arr, 8),
        low_limit_price: get_decimal(arr, 9),
        high_52_week_price: get_decimal(arr, 10),
        low_52_week_price: get_decimal(arr, 11),
        beta: None,
        earnings_per_share: None,
        dividend_frequency: None,
        ex_dividend_amount: None,
        ex_dividend_day_id: None,
        shares: None,
        free_float: None,
    })
}

/// Parse a Candle event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, eventFlags, index, time, sequence, count,
/// open, high, low, close, volume, vwap, bidVolume, askVolume, impVolatility, openInterest
pub fn parse_candle(arr: &[serde_json::Value]) -> Option<Candle> {
    if arr.len() < 2 {
        return None;
    }

    Some(Candle {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: get_i32(arr, 2),
        index: get_i64(arr, 3),
        time: get_i64(arr, 4),
        sequence: get_i64(arr, 5),
        count: get_i64(arr, 6),
        open: get_decimal(arr, 7),
        high: get_decimal(arr, 8),
        low: get_decimal(arr, 9),
        close: get_decimal(arr, 10),
        volume: get_decimal(arr, 11),
        vwap: get_decimal(arr, 12),
        bid_volume: get_decimal(arr, 13),
        ask_volume: get_decimal(arr, 14),
        imp_volatility: get_decimal(arr, 15),
        open_interest: get_i64(arr, 16),
    })
}

/// Parse a TheoPrice event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, eventFlags, index, time, sequence,
/// price, underlyingPrice, delta, gamma, dividend, interest
pub fn parse_theo_price(arr: &[serde_json::Value]) -> Option<TheoPrice> {
    if arr.len() < 2 {
        return None;
    }

    Some(TheoPrice {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: get_i32(arr, 2),
        index: get_i64(arr, 3),
        time: get_i64(arr, 4),
        sequence: get_i64(arr, 5),
        price: get_decimal(arr, 6),
        underlying_price: get_decimal(arr, 7),
        delta: get_decimal(arr, 8),
        gamma: get_decimal(arr, 9),
        dividend: get_decimal(arr, 10),
        interest: get_decimal(arr, 11),
    })
}

/// Parse a TimeAndSale event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, eventFlags, index, time, timeNanoPart,
/// sequence, exchangeCode, price, size, bidPrice, askPrice, exchangeSaleConditions,
/// aggressorSide, spreadLeg, extendedTradingHours, validTick, type
pub fn parse_time_and_sale(arr: &[serde_json::Value]) -> Option<TimeAndSale> {
    if arr.len() < 2 {
        return None;
    }

    Some(TimeAndSale {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: get_i32(arr, 2),
        index: get_i64(arr, 3),
        time: get_i64(arr, 4),
        time_nano_part: get_i32(arr, 5),
        sequence: get_i64(arr, 6),
        exchange_code: get_string(arr, 7),
        price: get_decimal(arr, 8),
        size: get_decimal(arr, 9),
        bid_price: get_decimal(arr, 10),
        ask_price: get_decimal(arr, 11),
        exchange_sale_conditions: get_string(arr, 12),
        trade_through_exempt: None,
        aggressor_side: get_string(arr, 13),
        spread_leg: get_bool(arr, 14),
        extended_trading_hours: get_bool(arr, 15),
        valid_tick: get_bool(arr, 16),
        trade_type: get_string(arr, 17),
    })
}

/// Parse a TradeETH event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, price, dayVolume, size
pub fn parse_trade_eth(arr: &[serde_json::Value]) -> Option<TradeETH> {
    if arr.len() < 2 {
        return None;
    }

    Some(TradeETH {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: None,
        index: None,
        time: None,
        time_nano_part: None,
        sequence: None,
        exchange_code: None,
        price: get_decimal(arr, 2),
        change: None,
        size: get_decimal(arr, 4),
        day_volume: get_decimal(arr, 3),
        day_turnover: None,
        tick_direction: None,
        extended_trading_hours: Some(true), // Always true for TradeETH
    })
}

/// Parse an Underlying event from COMPACT format.
///
/// Expected field order: eventType, eventSymbol, eventFlags, sequence, volatility,
/// frontVolatility, backVolatility, callVolume, putVolume, putCallRatio
pub fn parse_underlying(arr: &[serde_json::Value]) -> Option<Underlying> {
    if arr.len() < 2 {
        return None;
    }

    Some(Underlying {
        event_symbol: get_string(arr, 1)?,
        event_time: None,
        event_flags: get_i32(arr, 2),
        sequence: get_i64(arr, 3),
        volatility: get_decimal(arr, 4),
        front_volatility: get_decimal(arr, 5),
        back_volatility: get_decimal(arr, 6),
        call_volume: get_decimal(arr, 7),
        put_volume: get_decimal(arr, 8),
        put_call_ratio: get_decimal(arr, 9),
    })
}

/// Parse COMPACT format FEED_DATA and return parsed events.
///
/// The data structure is: `["EventType", [event1_data, event2_data, ...]]`
/// where each event_data is an array of values in field order.
pub fn parse_compact_feed_data(data: &serde_json::Value) -> Vec<super::DxEvent> {
    let mut events = Vec::new();

    let arr = match data.as_array() {
        Some(a) => a,
        None => return events,
    };

    // First element is the event type
    let event_type = match arr.first().and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return events,
    };

    // Second element is the array of event data arrays
    let event_data = match arr.get(1).and_then(|v| v.as_array()) {
        Some(d) => d,
        None => {
            // Sometimes the data is inline (single event format)
            // Try to parse the entire array as a single event
            if let Some(parsed) = parse_single_event(event_type, arr) {
                events.push(parsed);
            }
            return events;
        }
    };

    // Parse each event in the data array
    // Events can be in chunks where each chunk is a complete event
    // We need to determine the chunk size based on event type
    let chunk_size = get_event_field_count(event_type);

    if chunk_size > 0 && event_data.len() >= chunk_size {
        // Data is flattened - chunk it into individual events
        for chunk in event_data.chunks(chunk_size) {
            if let Some(parsed) = parse_single_event(event_type, chunk) {
                events.push(parsed);
            }
        }
    } else {
        // Try parsing as nested arrays
        for item in event_data {
            if let Some(item_arr) = item.as_array() {
                if let Some(parsed) = parse_single_event(event_type, item_arr) {
                    events.push(parsed);
                }
            }
        }
    }

    events
}

/// Get the number of fields for a given event type.
fn get_event_field_count(event_type: &str) -> usize {
    match event_type {
        "Quote" => fields::QUOTE_FIELDS.len(),
        "Trade" => fields::TRADE_FIELDS.len(),
        "TradeETH" => fields::TRADE_ETH_FIELDS.len(),
        "Greeks" => fields::GREEKS_FIELDS.len(),
        "Summary" => fields::SUMMARY_FIELDS.len(),
        "Profile" => fields::PROFILE_FIELDS.len(),
        "Candle" => fields::CANDLE_FIELDS.len(),
        "TheoPrice" => fields::THEO_PRICE_FIELDS.len(),
        "TimeAndSale" => fields::TIME_AND_SALE_FIELDS.len(),
        "Underlying" => fields::UNDERLYING_FIELDS.len(),
        _ => 0,
    }
}

/// Parse a single event from its data array.
fn parse_single_event(event_type: &str, data: &[serde_json::Value]) -> Option<super::DxEvent> {
    match event_type {
        "Quote" => parse_quote(data).map(super::DxEvent::Quote),
        "Trade" => parse_trade(data).map(super::DxEvent::Trade),
        "TradeETH" => parse_trade_eth(data).map(super::DxEvent::TradeETH),
        "Greeks" => parse_greeks(data).map(super::DxEvent::Greeks),
        "Summary" => parse_summary(data).map(super::DxEvent::Summary),
        "Profile" => parse_profile(data).map(super::DxEvent::Profile),
        "Candle" => parse_candle(data).map(super::DxEvent::Candle),
        "TheoPrice" => parse_theo_price(data).map(super::DxEvent::TheoPrice),
        "TimeAndSale" => parse_time_and_sale(data).map(super::DxEvent::TimeAndSale),
        "Underlying" => parse_underlying(data).map(super::DxEvent::Underlying),
        _ => None,
    }
}

/// Get the acceptEventFields for FEED_SETUP message.
pub fn get_accept_event_fields() -> serde_json::Value {
    serde_json::json!({
        "Quote": fields::QUOTE_FIELDS,
        "Trade": fields::TRADE_FIELDS,
        "TradeETH": fields::TRADE_ETH_FIELDS,
        "Greeks": fields::GREEKS_FIELDS,
        "Summary": fields::SUMMARY_FIELDS,
        "Profile": fields::PROFILE_FIELDS,
        "Candle": fields::CANDLE_FIELDS,
        "TheoPrice": fields::THEO_PRICE_FIELDS,
        "TimeAndSale": fields::TIME_AND_SALE_FIELDS,
        "Underlying": fields::UNDERLYING_FIELDS
    })
}

/// Get the event type enum from a string.
pub fn event_type_from_str(s: &str) -> Option<EventType> {
    match s {
        "Quote" => Some(EventType::Quote),
        "Trade" => Some(EventType::Trade),
        "TradeETH" => Some(EventType::TradeETH),
        "Greeks" => Some(EventType::Greeks),
        "Summary" => Some(EventType::Summary),
        "Profile" => Some(EventType::Profile),
        "Candle" => Some(EventType::Candle),
        "TheoPrice" => Some(EventType::TheoPrice),
        "TimeAndSale" => Some(EventType::TimeAndSale),
        "Underlying" => Some(EventType::Underlying),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_parse_quote_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Quote"),
            serde_json::json!("SPY"),
            serde_json::json!(450.25),
            serde_json::json!(450.30),
            serde_json::json!(100),
            serde_json::json!(200),
        ];

        let quote = parse_quote(&data).unwrap();
        assert_eq!(quote.event_symbol, "SPY");
        assert_eq!(quote.bid_price, Some(dec!(450.25)));
        assert_eq!(quote.ask_price, Some(dec!(450.30)));
        assert_eq!(quote.bid_size, Some(dec!(100)));
        assert_eq!(quote.ask_size, Some(dec!(200)));
    }

    #[test]
    fn test_parse_quote_with_nan() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Quote"),
            serde_json::json!("SPY"),
            serde_json::json!("NaN"),
            serde_json::json!(450.30),
            serde_json::json!("NaN"),
            serde_json::json!(200),
        ];

        let quote = parse_quote(&data).unwrap();
        assert_eq!(quote.event_symbol, "SPY");
        assert_eq!(quote.bid_price, None); // NaN becomes None
        assert_eq!(quote.ask_price, Some(dec!(450.30)));
        assert_eq!(quote.bid_size, None);
        assert_eq!(quote.ask_size, Some(dec!(200)));
    }

    #[test]
    fn test_parse_trade_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Trade"),
            serde_json::json!("AAPL"),
            serde_json::json!(175.50),
            serde_json::json!(1000000),
            serde_json::json!(100),
        ];

        let trade = parse_trade(&data).unwrap();
        assert_eq!(trade.event_symbol, "AAPL");
        assert_eq!(trade.price, Some(dec!(175.50)));
        assert_eq!(trade.day_volume, Some(dec!(1000000)));
        assert_eq!(trade.size, Some(dec!(100)));
    }

    #[test]
    fn test_parse_greeks_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Greeks"),
            serde_json::json!(".SPY230120C450"),
            serde_json::json!(0.25),   // volatility
            serde_json::json!(0.55),   // delta
            serde_json::json!(0.02),   // gamma
            serde_json::json!(-0.05),  // theta
            serde_json::json!(0.01),   // rho
            serde_json::json!(0.15),   // vega
        ];

        let greeks = parse_greeks(&data).unwrap();
        assert_eq!(greeks.event_symbol, ".SPY230120C450");
        assert_eq!(greeks.volatility, Some(dec!(0.25)));
        assert_eq!(greeks.delta, Some(dec!(0.55)));
        assert_eq!(greeks.gamma, Some(dec!(0.02)));
        assert_eq!(greeks.theta, Some(dec!(-0.05)));
        assert_eq!(greeks.rho, Some(dec!(0.01)));
        assert_eq!(greeks.vega, Some(dec!(0.15)));
    }

    #[test]
    fn test_parse_summary_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Summary"),
            serde_json::json!("SPY"),
            serde_json::json!(50000),   // openInterest
            serde_json::json!(449.00),  // dayOpenPrice
            serde_json::json!(451.00),  // dayHighPrice
            serde_json::json!(448.50),  // dayLowPrice
            serde_json::json!(450.00),  // prevDayClosePrice
        ];

        let summary = parse_summary(&data).unwrap();
        assert_eq!(summary.event_symbol, "SPY");
        assert_eq!(summary.open_interest, Some(50000));
        assert_eq!(summary.day_open_price, Some(dec!(449.00)));
        assert_eq!(summary.day_high_price, Some(dec!(451.00)));
        assert_eq!(summary.day_low_price, Some(dec!(448.50)));
        assert_eq!(summary.prev_day_close_price, Some(dec!(450.00)));
    }

    #[test]
    fn test_parse_profile_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Profile"),
            serde_json::json!("AAPL"),
            serde_json::json!("Apple Inc."),
            serde_json::json!("NONE"),
            serde_json::json!("ACTIVE"),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(198.23),
            serde_json::json!(124.17),
        ];

        let profile = parse_profile(&data).unwrap();
        assert_eq!(profile.event_symbol, "AAPL");
        assert_eq!(profile.description, Some("Apple Inc.".to_string()));
        assert_eq!(profile.short_sale_restriction, Some("NONE".to_string()));
        assert_eq!(profile.trading_status, Some("ACTIVE".to_string()));
        assert_eq!(profile.high_52_week_price, Some(dec!(198.23)));
        assert_eq!(profile.low_52_week_price, Some(dec!(124.17)));
    }

    #[test]
    fn test_parse_candle_compact() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Candle"),
            serde_json::json!("AAPL{=1d}"),
            serde_json::json!(0),        // eventFlags
            serde_json::json!(12345678), // index
            serde_json::json!(1704067200000i64), // time
            serde_json::json!(1),        // sequence
            serde_json::json!(1000),     // count
            serde_json::json!(175.00),   // open
            serde_json::json!(180.00),   // high
            serde_json::json!(174.00),   // low
            serde_json::json!(178.50),   // close
            serde_json::json!(5000000),  // volume
            serde_json::json!(177.25),   // vwap
            serde_json::json!(2500000),  // bidVolume
            serde_json::json!(2500000),  // askVolume
            serde_json::json!(0.30),     // impVolatility
            serde_json::json!(100000),   // openInterest
        ];

        let candle = parse_candle(&data).unwrap();
        assert_eq!(candle.event_symbol, "AAPL{=1d}");
        assert_eq!(candle.open, Some(dec!(175.00)));
        assert_eq!(candle.high, Some(dec!(180.00)));
        assert_eq!(candle.low, Some(dec!(174.00)));
        assert_eq!(candle.close, Some(dec!(178.50)));
        assert_eq!(candle.volume, Some(dec!(5000000)));
    }

    #[test]
    fn test_parse_compact_feed_data_multiple_events() {
        use crate::streaming::dxlink::DxEvent;

        // Simulate FEED_DATA with multiple Quote events in flattened array
        let data = serde_json::json!([
            "Quote",
            [
                "Quote", "SPY", 450.25, 450.30, 100, 200,
                "Quote", "AAPL", 175.00, 175.05, 50, 75
            ]
        ]);

        let events = parse_compact_feed_data(&data);
        assert_eq!(events.len(), 2);

        if let DxEvent::Quote(q1) = &events[0] {
            assert_eq!(q1.event_symbol, "SPY");
        } else {
            panic!("Expected Quote event");
        }

        if let DxEvent::Quote(q2) = &events[1] {
            assert_eq!(q2.event_symbol, "AAPL");
        } else {
            panic!("Expected Quote event");
        }
    }

    #[test]
    fn test_parse_compact_feed_data_single_event() {
        use crate::streaming::dxlink::DxEvent;

        // Single event format (inline)
        let data = serde_json::json!([
            "Trade", "SPY", 559.36, 13743299.0, 100.0
        ]);

        let events = parse_compact_feed_data(&data);
        assert_eq!(events.len(), 1);

        if let DxEvent::Trade(t) = &events[0] {
            assert_eq!(t.event_symbol, "SPY");
            assert_eq!(t.price, Some(dec!(559.36)));
        } else {
            panic!("Expected Trade event");
        }
    }

    #[test]
    fn test_get_decimal_with_scientific_notation() {
        let arr = vec![serde_json::json!(1.3743299E7)];
        let result = get_decimal(&arr, 0);
        assert!(result.is_some());
        // 1.3743299E7 = 13743299
        assert!(result.unwrap() > dec!(13000000));
    }

    #[test]
    fn test_get_accept_event_fields() {
        let fields = get_accept_event_fields();
        assert!(fields.get("Quote").is_some());
        assert!(fields.get("Trade").is_some());
        assert!(fields.get("Greeks").is_some());
        assert!(fields.get("Summary").is_some());
        assert!(fields.get("Profile").is_some());
    }

    #[test]
    fn test_event_type_from_str() {
        assert_eq!(event_type_from_str("Quote"), Some(EventType::Quote));
        assert_eq!(event_type_from_str("Trade"), Some(EventType::Trade));
        assert_eq!(event_type_from_str("TradeETH"), Some(EventType::TradeETH));
        assert_eq!(event_type_from_str("Greeks"), Some(EventType::Greeks));
        assert_eq!(event_type_from_str("TimeAndSale"), Some(EventType::TimeAndSale));
        assert_eq!(event_type_from_str("Underlying"), Some(EventType::Underlying));
        assert_eq!(event_type_from_str("Unknown"), None);
    }

    #[test]
    fn test_parse_empty_data() {
        let data = serde_json::json!([]);
        let events = parse_compact_feed_data(&data);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_null_values() {
        let data: Vec<serde_json::Value> = vec![
            serde_json::json!("Quote"),
            serde_json::json!("SPY"),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(null),
            serde_json::json!(null),
        ];

        let quote = parse_quote(&data).unwrap();
        assert_eq!(quote.event_symbol, "SPY");
        assert_eq!(quote.bid_price, None);
        assert_eq!(quote.ask_price, None);
    }
}
