//! Account notification types.
//!
//! These types represent real-time account activity notifications.

use rust_decimal::Decimal;
use serde::Deserialize;

/// Account notification event.
#[derive(Debug, Clone)]
pub enum AccountNotification {
    /// Heartbeat message to keep connection alive
    Heartbeat,
    /// Subscription confirmation
    SubscriptionConfirmation {
        /// List of subscribed account numbers
        accounts: Vec<String>,
    },
    /// Order status update
    Order(OrderNotification),
    /// Position update
    Position(PositionNotification),
    /// Balance update
    Balance(BalanceNotification),
    /// Quote alert triggered
    QuoteAlert(QuoteAlertNotification),
    /// Unknown notification type (raw JSON)
    Unknown(serde_json::Value),
}

/// Order notification data.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderNotification {
    /// Account number
    pub account_number: Option<String>,
    /// Order ID
    pub id: Option<i64>,
    /// Order status
    pub status: Option<String>,
    /// Underlying symbol
    pub underlying_symbol: Option<String>,
    /// Order type
    pub order_type: Option<String>,
    /// Time in force
    pub time_in_force: Option<String>,
    /// Price (for limit orders)
    pub price: Option<Decimal>,
    /// Stop trigger price
    pub stop_trigger: Option<Decimal>,
    /// Order size
    pub size: Option<i32>,
    /// Filled quantity
    pub filled_quantity: Option<Decimal>,
    /// Remaining quantity
    pub remaining_quantity: Option<Decimal>,
    /// Average fill price
    pub average_fill_price: Option<Decimal>,
    /// Order legs
    pub legs: Option<Vec<OrderLegNotification>>,
    /// Cancellable flag
    pub cancellable: Option<bool>,
    /// Editable flag
    pub editable: Option<bool>,
    /// Edited flag
    pub edited: Option<bool>,
    /// Updated at timestamp
    pub updated_at: Option<String>,
    /// Received at timestamp
    pub received_at: Option<String>,
}

/// Order leg notification data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderLegNotification {
    /// Instrument type
    pub instrument_type: Option<String>,
    /// Symbol
    pub symbol: Option<String>,
    /// Action (buy/sell)
    pub action: Option<String>,
    /// Quantity
    pub quantity: Option<i32>,
    /// Remaining quantity
    pub remaining_quantity: Option<i32>,
    /// Fills for this leg
    pub fills: Option<Vec<FillNotification>>,
}

/// Fill notification data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FillNotification {
    /// Fill ID
    pub id: Option<String>,
    /// Fill quantity
    pub quantity: Option<Decimal>,
    /// Fill price
    pub fill_price: Option<Decimal>,
    /// Filled at timestamp
    pub filled_at: Option<String>,
    /// Destination venue
    pub destination_venue: Option<String>,
}

/// Position notification data.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PositionNotification {
    /// Account number
    pub account_number: Option<String>,
    /// Symbol
    pub symbol: Option<String>,
    /// Underlying symbol
    pub underlying_symbol: Option<String>,
    /// Instrument type
    pub instrument_type: Option<String>,
    /// Quantity
    pub quantity: Option<Decimal>,
    /// Quantity direction (long/short)
    pub quantity_direction: Option<String>,
    /// Average open price
    pub average_open_price: Option<Decimal>,
    /// Average yearly market close
    pub average_yearly_market_close_price: Option<Decimal>,
    /// Close price
    pub close_price: Option<Decimal>,
    /// Mark price
    pub mark_price: Option<Decimal>,
    /// Mark at close
    pub mark: Option<Decimal>,
    /// Cost basis
    pub cost_effect: Option<String>,
    /// Realized day gain
    pub realized_day_gain: Option<Decimal>,
    /// Realized day gain effect
    pub realized_day_gain_effect: Option<String>,
    /// Realized day gain date
    pub realized_day_gain_date: Option<String>,
    /// Realized today gain
    pub realized_today: Option<Decimal>,
    /// Realized today effect
    pub realized_today_effect: Option<String>,
    /// Realized today date
    pub realized_today_date: Option<String>,
    /// Expires at (for options)
    pub expires_at: Option<String>,
    /// Created at timestamp
    pub created_at: Option<String>,
    /// Updated at timestamp
    pub updated_at: Option<String>,
}

/// Balance notification data.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BalanceNotification {
    /// Account number
    pub account_number: Option<String>,
    /// Cash balance
    pub cash_balance: Option<Decimal>,
    /// Long equity value
    pub long_equity_value: Option<Decimal>,
    /// Short equity value
    pub short_equity_value: Option<Decimal>,
    /// Long derivative value
    pub long_derivative_value: Option<Decimal>,
    /// Short derivative value
    pub short_derivative_value: Option<Decimal>,
    /// Long futures value
    pub long_futures_value: Option<Decimal>,
    /// Short futures value
    pub short_futures_value: Option<Decimal>,
    /// Long futures derivative value
    pub long_futures_derivative_value: Option<Decimal>,
    /// Short futures derivative value
    pub short_futures_derivative_value: Option<Decimal>,
    /// Long margineable value
    pub long_margineable_value: Option<Decimal>,
    /// Short margineable value
    pub short_margineable_value: Option<Decimal>,
    /// Margin equity
    pub margin_equity: Option<Decimal>,
    /// Equity buying power
    pub equity_buying_power: Option<Decimal>,
    /// Derivative buying power
    pub derivative_buying_power: Option<Decimal>,
    /// Day trading buying power
    pub day_trading_buying_power: Option<Decimal>,
    /// Futures margin requirement
    pub futures_margin_requirement: Option<Decimal>,
    /// Available trading funds
    pub available_trading_funds: Option<Decimal>,
    /// Maintenance requirement
    pub maintenance_requirement: Option<Decimal>,
    /// Maintenance call value
    pub maintenance_call_value: Option<Decimal>,
    /// Regulation T call value
    pub reg_t_call_value: Option<Decimal>,
    /// Day trading call value
    pub day_trading_call_value: Option<Decimal>,
    /// Day equity call value
    pub day_equity_call_value: Option<Decimal>,
    /// Net liquidating value
    pub net_liquidating_value: Option<Decimal>,
    /// Cash available to withdraw
    pub cash_available_to_withdraw: Option<Decimal>,
    /// Day trade excess
    pub day_trade_excess: Option<Decimal>,
    /// Pending cash
    pub pending_cash: Option<Decimal>,
    /// Pending cash effect
    pub pending_cash_effect: Option<String>,
    /// Snapshot date
    pub snapshot_date: Option<String>,
    /// Reg T margin requirement
    pub reg_t_margin_requirement: Option<Decimal>,
    /// Futures overnight margin requirement
    pub futures_overnight_margin_requirement: Option<Decimal>,
    /// Futures intraday margin requirement
    pub futures_intraday_margin_requirement: Option<Decimal>,
    /// Maintenance excess
    pub maintenance_excess: Option<Decimal>,
    /// Pending margin interest
    pub pending_margin_interest: Option<Decimal>,
    /// Effective cryptocurrency buying power
    pub effective_cryptocurrency_buying_power: Option<Decimal>,
    /// Updated at timestamp
    pub updated_at: Option<String>,
}

/// Quote alert notification.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QuoteAlertNotification {
    /// Alert ID
    pub id: Option<i64>,
    /// User external ID
    pub user_external_id: Option<String>,
    /// Symbol
    pub symbol: Option<String>,
    /// Field being watched (e.g., "last", "bid", "ask")
    pub field: Option<String>,
    /// Operator (e.g., "gte", "lte")
    pub operator: Option<String>,
    /// Threshold value
    pub threshold: Option<Decimal>,
    /// Threshold value (string variant)
    pub threshold_str: Option<String>,
    /// Whether alert was triggered
    pub triggered: Option<bool>,
    /// Triggered value
    pub triggered_value: Option<Decimal>,
    /// Triggered at timestamp
    pub triggered_at: Option<String>,
    /// Created at timestamp
    pub created_at: Option<String>,
    /// Completed at timestamp
    pub completed_at: Option<String>,
}
