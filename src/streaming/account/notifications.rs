//! Account notification types.
//!
//! These types represent real-time account activity notifications.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

use crate::models::OrderStatus;

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

    // ===== Connection Lifecycle Events (Gap 3) =====

    /// WebSocket connection was lost.
    ///
    /// After receiving this, no further notifications will arrive until
    /// reconnection succeeds (if auto-reconnect is enabled) or `reconnect()`
    /// is called manually.
    Disconnected {
        /// Reason for disconnection
        reason: String,
    },

    /// Successfully reconnected to the streaming service.
    ///
    /// All previous subscriptions have been automatically restored.
    Reconnected {
        /// Number of accounts re-subscribed
        accounts_restored: usize,
    },

    /// Connection health warning.
    ///
    /// Sent when heartbeat responses are delayed or connection quality degrades.
    ConnectionWarning {
        /// Warning message
        message: String,
    },

    /// Unknown notification type (raw JSON).
    ///
    /// **Important**: This variant indicates the API sent a notification type
    /// that this library doesn't recognize. This could indicate:
    /// - A new API feature not yet supported by this library
    /// - An API change that requires a library update
    ///
    /// Applications should log these events and consider reporting them.
    Unknown(serde_json::Value),

    /// Failed to parse notification data.
    ///
    /// This variant is emitted when the notification action is recognized but
    /// the data payload could not be deserialized. This is a critical error
    /// that may indicate data loss - applications should handle this carefully.
    ParseError {
        /// The notification action type that failed to parse
        action: String,
        /// Error message describing what went wrong
        error: String,
        /// Raw JSON data that failed to parse
        raw_data: serde_json::Value,
    },
}

impl AccountNotification {
    /// Returns `true` if this is a connection lifecycle event.
    ///
    /// Connection events include `Disconnected`, `Reconnected`, and `ConnectionWarning`.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::AccountNotification;
    ///
    /// let disconnected = AccountNotification::Disconnected {
    ///     reason: "Connection reset".to_string()
    /// };
    /// assert!(disconnected.is_connection_event());
    ///
    /// let heartbeat = AccountNotification::Heartbeat;
    /// assert!(!heartbeat.is_connection_event());
    /// ```
    pub fn is_connection_event(&self) -> bool {
        matches!(
            self,
            AccountNotification::Disconnected { .. }
                | AccountNotification::Reconnected { .. }
                | AccountNotification::ConnectionWarning { .. }
        )
    }

    /// Returns `true` if this notification indicates the connection is healthy.
    ///
    /// Healthy events include `Heartbeat`, `SubscriptionConfirmation`, and `Reconnected`.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::AccountNotification;
    ///
    /// let heartbeat = AccountNotification::Heartbeat;
    /// assert!(heartbeat.is_healthy());
    ///
    /// let disconnected = AccountNotification::Disconnected {
    ///     reason: "test".to_string()
    /// };
    /// assert!(!disconnected.is_healthy());
    /// ```
    pub fn is_healthy(&self) -> bool {
        matches!(
            self,
            AccountNotification::Heartbeat
                | AccountNotification::SubscriptionConfirmation { .. }
                | AccountNotification::Reconnected { .. }
        )
    }

    /// Returns `true` if this is an order-related notification.
    pub fn is_order(&self) -> bool {
        matches!(self, AccountNotification::Order(_))
    }

    /// Returns `true` if this is a position-related notification.
    pub fn is_position(&self) -> bool {
        matches!(self, AccountNotification::Position(_))
    }

    /// Returns `true` if this is a balance-related notification.
    pub fn is_balance(&self) -> bool {
        matches!(self, AccountNotification::Balance(_))
    }

    /// Returns `true` if this is an unknown notification type.
    ///
    /// Unknown notifications should be logged and monitored as they may indicate
    /// API changes that require a library update.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::AccountNotification;
    ///
    /// let unknown = AccountNotification::Unknown(serde_json::json!({"action": "new-feature"}));
    /// assert!(unknown.is_unknown());
    ///
    /// let heartbeat = AccountNotification::Heartbeat;
    /// assert!(!heartbeat.is_unknown());
    /// ```
    pub fn is_unknown(&self) -> bool {
        matches!(self, AccountNotification::Unknown(_))
    }

    /// Returns `true` if this is a parse error notification.
    ///
    /// Parse errors indicate that a known notification type could not be deserialized.
    /// This is a critical condition that may indicate data loss.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::AccountNotification;
    ///
    /// let parse_error = AccountNotification::ParseError {
    ///     action: "order".to_string(),
    ///     error: "missing field".to_string(),
    ///     raw_data: serde_json::json!({}),
    /// };
    /// assert!(parse_error.is_parse_error());
    /// ```
    pub fn is_parse_error(&self) -> bool {
        matches!(self, AccountNotification::ParseError { .. })
    }

    /// Returns `true` if this notification indicates an error condition.
    ///
    /// Error conditions include `Disconnected`, `ConnectionWarning`, `Unknown`, and `ParseError`.
    /// These should be handled specially by applications.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::AccountNotification;
    ///
    /// let parse_error = AccountNotification::ParseError {
    ///     action: "order".to_string(),
    ///     error: "missing field".to_string(),
    ///     raw_data: serde_json::json!({}),
    /// };
    /// assert!(parse_error.is_error_condition());
    ///
    /// let heartbeat = AccountNotification::Heartbeat;
    /// assert!(!heartbeat.is_error_condition());
    /// ```
    pub fn is_error_condition(&self) -> bool {
        matches!(
            self,
            AccountNotification::Disconnected { .. }
                | AccountNotification::ConnectionWarning { .. }
                | AccountNotification::Unknown(_)
                | AccountNotification::ParseError { .. }
        )
    }
}

/// Order notification data.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderNotification {
    /// Account number
    pub account_number: Option<String>,
    /// Order ID
    pub id: Option<i64>,
    /// Order status (typed).
    ///
    /// This is deserialized from the string status in the JSON response.
    /// If the status string doesn't match a known variant, this will be `None`.
    #[serde(default, deserialize_with = "deserialize_order_status")]
    pub status: Option<OrderStatus>,
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

/// Custom deserializer for OrderStatus that handles unknown variants gracefully.
fn deserialize_order_status<'de, D>(deserializer: D) -> Result<Option<OrderStatus>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;

    match opt {
        Some(s) => {
            // Try to parse the status string into OrderStatus
            match serde_json::from_value::<OrderStatus>(serde_json::Value::String(s.clone())) {
                Ok(status) => Ok(Some(status)),
                Err(_) => {
                    // Unknown status - return None but don't fail deserialization
                    Ok(None)
                }
            }
        }
        None => Ok(None),
    }
}

impl OrderNotification {
    // ===== Order ID Helper Methods (Gap 4) =====

    /// Get the order ID as a string.
    ///
    /// Returns `None` if the order notification doesn't contain an ID
    /// (which should be rare for valid order updates).
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    ///
    /// let order = OrderNotification::default();
    /// assert!(order.order_id().is_none());
    /// ```
    pub fn order_id(&self) -> Option<String> {
        self.id.map(|id| id.to_string())
    }

    /// Get the order ID as i64.
    ///
    /// This is a direct accessor for the numeric ID field.
    pub fn order_id_numeric(&self) -> Option<i64> {
        self.id
    }

    /// Check if this notification matches a specific order ID (string variant).
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    ///
    /// let mut order = OrderNotification::default();
    /// order.id = Some(12345);
    /// assert!(order.matches_order_id("12345"));
    /// assert!(!order.matches_order_id("99999"));
    /// ```
    pub fn matches_order_id(&self, order_id: &str) -> bool {
        self.order_id().as_deref() == Some(order_id)
    }

    /// Check if this notification matches a specific order ID (i64 variant).
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    ///
    /// let mut order = OrderNotification::default();
    /// order.id = Some(12345);
    /// assert!(order.matches_order(12345));
    /// assert!(!order.matches_order(99999));
    /// ```
    pub fn matches_order(&self, order_id: i64) -> bool {
        self.id == Some(order_id)
    }

    // ===== Typed Order Status Methods (Gap 5) =====

    /// Check if this order is in a terminal state.
    ///
    /// Terminal states include: Filled, Cancelled, Expired, Rejected, Removed, PartiallyRemoved.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    /// use tastytrade_rs::models::OrderStatus;
    ///
    /// let mut order = OrderNotification::default();
    /// order.status = Some(OrderStatus::Filled);
    /// assert!(order.is_terminal());
    ///
    /// order.status = Some(OrderStatus::Live);
    /// assert!(!order.is_terminal());
    /// ```
    pub fn is_terminal(&self) -> bool {
        self.status.as_ref().map(|s| s.is_terminal()).unwrap_or(false)
    }

    /// Check if this order is working (live, in-flight, etc.).
    ///
    /// Working states include: Received, Routed, InFlight, Live, Contingent.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    /// use tastytrade_rs::models::OrderStatus;
    ///
    /// let mut order = OrderNotification::default();
    /// order.status = Some(OrderStatus::Live);
    /// assert!(order.is_working());
    ///
    /// order.status = Some(OrderStatus::Filled);
    /// assert!(!order.is_working());
    /// ```
    pub fn is_working(&self) -> bool {
        self.status.as_ref().map(|s| s.is_working()).unwrap_or(false)
    }

    /// Check if this order was completely filled.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::OrderNotification;
    /// use tastytrade_rs::models::OrderStatus;
    ///
    /// let mut order = OrderNotification::default();
    /// order.status = Some(OrderStatus::Filled);
    /// assert!(order.is_filled());
    /// ```
    pub fn is_filled(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Filled))
    }

    /// Check if this order was rejected.
    pub fn is_rejected(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Rejected))
    }

    /// Check if this order was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Cancelled))
    }

    /// Check if this order expired.
    pub fn is_expired(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Expired))
    }

    /// Check if this order is live and working on the exchange.
    pub fn is_live(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Live))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Phase 1 Tests: Order ID Helper Methods (Gap 4) =====

    #[test]
    fn test_order_id_returns_string() {
        let mut order = OrderNotification::default();
        order.id = Some(12345);
        assert_eq!(order.order_id(), Some("12345".to_string()));
    }

    #[test]
    fn test_order_id_returns_none_when_missing() {
        let order = OrderNotification::default();
        assert!(order.order_id().is_none());
    }

    #[test]
    fn test_order_id_numeric() {
        let mut order = OrderNotification::default();
        order.id = Some(99999);
        assert_eq!(order.order_id_numeric(), Some(99999));
    }

    #[test]
    fn test_matches_order_id_string() {
        let mut order = OrderNotification::default();
        order.id = Some(12345);
        assert!(order.matches_order_id("12345"));
        assert!(!order.matches_order_id("99999"));
        assert!(!order.matches_order_id(""));
    }

    #[test]
    fn test_matches_order_i64() {
        let mut order = OrderNotification::default();
        order.id = Some(12345);
        assert!(order.matches_order(12345));
        assert!(!order.matches_order(99999));
        assert!(!order.matches_order(0));
    }

    #[test]
    fn test_matches_order_when_id_missing() {
        let order = OrderNotification::default();
        assert!(!order.matches_order_id("12345"));
        assert!(!order.matches_order(12345));
    }

    // ===== Phase 1 Tests: Typed Order Status (Gap 5) =====

    #[test]
    fn test_is_filled() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Filled);
        assert!(order.is_filled());
        assert!(!order.is_rejected());
        assert!(!order.is_cancelled());
    }

    #[test]
    fn test_is_rejected() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Rejected);
        assert!(order.is_rejected());
        assert!(!order.is_filled());
    }

    #[test]
    fn test_is_cancelled() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Cancelled);
        assert!(order.is_cancelled());
        assert!(!order.is_filled());
    }

    #[test]
    fn test_is_expired() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Expired);
        assert!(order.is_expired());
    }

    #[test]
    fn test_is_live() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Live);
        assert!(order.is_live());
        assert!(!order.is_filled());
    }

    #[test]
    fn test_is_terminal_for_filled() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Filled);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_is_terminal_for_cancelled() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Cancelled);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_is_terminal_for_rejected() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Rejected);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_is_terminal_for_expired() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Expired);
        assert!(order.is_terminal());
    }

    #[test]
    fn test_is_not_terminal_for_live() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Live);
        assert!(!order.is_terminal());
    }

    #[test]
    fn test_is_not_terminal_for_received() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Received);
        assert!(!order.is_terminal());
    }

    #[test]
    fn test_is_working_for_live() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Live);
        assert!(order.is_working());
    }

    #[test]
    fn test_is_working_for_received() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Received);
        assert!(order.is_working());
    }

    #[test]
    fn test_is_working_for_routed() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Routed);
        assert!(order.is_working());
    }

    #[test]
    fn test_is_not_working_for_filled() {
        let mut order = OrderNotification::default();
        order.status = Some(OrderStatus::Filled);
        assert!(!order.is_working());
    }

    #[test]
    fn test_status_none_is_not_terminal() {
        let order = OrderNotification::default();
        assert!(!order.is_terminal());
    }

    #[test]
    fn test_status_none_is_not_working() {
        let order = OrderNotification::default();
        assert!(!order.is_working());
    }

    // ===== Phase 1 Tests: Deserialization =====

    #[test]
    fn test_deserialize_order_notification_with_filled_status() {
        let json = r#"{
            "id": 12345,
            "account-number": "5WY00001",
            "status": "Filled",
            "underlying-symbol": "AAPL"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.id, Some(12345));
        assert_eq!(order.account_number, Some("5WY00001".to_string()));
        assert_eq!(order.status, Some(OrderStatus::Filled));
        assert!(order.is_filled());
        assert!(order.is_terminal());
    }

    #[test]
    fn test_deserialize_order_notification_with_live_status() {
        let json = r#"{
            "id": 99999,
            "status": "Live"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(OrderStatus::Live));
        assert!(order.is_live());
        assert!(order.is_working());
        assert!(!order.is_terminal());
    }

    #[test]
    fn test_deserialize_order_notification_with_rejected_status() {
        let json = r#"{
            "id": 11111,
            "status": "Rejected"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(OrderStatus::Rejected));
        assert!(order.is_rejected());
        assert!(order.is_terminal());
    }

    #[test]
    fn test_deserialize_order_notification_with_unknown_status() {
        // Unknown status should result in None, not a deserialization error
        let json = r#"{
            "id": 22222,
            "status": "SomeUnknownStatus"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.id, Some(22222));
        assert!(order.status.is_none());
        assert!(!order.is_terminal());
        assert!(!order.is_working());
    }

    #[test]
    fn test_deserialize_order_notification_without_status() {
        let json = r#"{
            "id": 33333,
            "underlying-symbol": "SPY"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.id, Some(33333));
        assert!(order.status.is_none());
    }

    #[test]
    fn test_deserialize_order_with_in_flight_status() {
        let json = r#"{
            "id": 44444,
            "status": "In Flight"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(OrderStatus::InFlight));
        assert!(order.is_working());
    }

    #[test]
    fn test_deserialize_order_with_cancel_requested_status() {
        let json = r#"{
            "id": 55555,
            "status": "Cancel Requested"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();
        assert_eq!(order.status, Some(OrderStatus::CancelRequested));
    }

    // ===== Combined Helper Tests =====

    #[test]
    fn test_order_matching_and_status_combined() {
        let json = r#"{
            "id": 12345,
            "account-number": "5WY00001",
            "status": "Filled",
            "underlying-symbol": "AAPL",
            "average-fill-price": "150.50"
        }"#;

        let order: OrderNotification = serde_json::from_str(json).unwrap();

        // Test order ID matching
        assert!(order.matches_order(12345));
        assert!(order.matches_order_id("12345"));
        assert!(!order.matches_order(99999));

        // Test status
        assert!(order.is_filled());
        assert!(order.is_terminal());
        assert!(!order.is_working());

        // Verify data
        assert_eq!(order.underlying_symbol, Some("AAPL".to_string()));
    }

    // ===== Phase 2 Tests: Connection Status Events (Gap 3) =====

    #[test]
    fn test_disconnected_is_connection_event() {
        let event = AccountNotification::Disconnected {
            reason: "Connection reset by peer".to_string(),
        };
        assert!(event.is_connection_event());
        assert!(!event.is_healthy());
    }

    #[test]
    fn test_reconnected_is_connection_event() {
        let event = AccountNotification::Reconnected {
            accounts_restored: 3,
        };
        assert!(event.is_connection_event());
        assert!(event.is_healthy()); // Reconnected is healthy!
    }

    #[test]
    fn test_connection_warning_is_connection_event() {
        let event = AccountNotification::ConnectionWarning {
            message: "Heartbeat delayed".to_string(),
        };
        assert!(event.is_connection_event());
        assert!(!event.is_healthy());
    }

    #[test]
    fn test_heartbeat_is_healthy() {
        let event = AccountNotification::Heartbeat;
        assert!(!event.is_connection_event());
        assert!(event.is_healthy());
    }

    #[test]
    fn test_subscription_confirmation_is_healthy() {
        let event = AccountNotification::SubscriptionConfirmation {
            accounts: vec!["5WY00001".to_string()],
        };
        assert!(!event.is_connection_event());
        assert!(event.is_healthy());
    }

    #[test]
    fn test_order_is_not_connection_event() {
        let event = AccountNotification::Order(OrderNotification::default());
        assert!(!event.is_connection_event());
        assert!(!event.is_healthy());
    }

    #[test]
    fn test_position_is_not_connection_event() {
        let event = AccountNotification::Position(PositionNotification::default());
        assert!(!event.is_connection_event());
    }

    #[test]
    fn test_balance_is_not_connection_event() {
        let event = AccountNotification::Balance(BalanceNotification::default());
        assert!(!event.is_connection_event());
    }

    #[test]
    fn test_is_order() {
        let order_event = AccountNotification::Order(OrderNotification::default());
        assert!(order_event.is_order());

        let heartbeat = AccountNotification::Heartbeat;
        assert!(!heartbeat.is_order());
    }

    #[test]
    fn test_is_position() {
        let position_event = AccountNotification::Position(PositionNotification::default());
        assert!(position_event.is_position());

        let order_event = AccountNotification::Order(OrderNotification::default());
        assert!(!order_event.is_position());
    }

    #[test]
    fn test_is_balance() {
        let balance_event = AccountNotification::Balance(BalanceNotification::default());
        assert!(balance_event.is_balance());

        let order_event = AccountNotification::Order(OrderNotification::default());
        assert!(!order_event.is_balance());
    }

    #[test]
    fn test_disconnected_reason_preserved() {
        let reason = "Network timeout after 30 seconds";
        let event = AccountNotification::Disconnected {
            reason: reason.to_string(),
        };

        if let AccountNotification::Disconnected { reason: r } = event {
            assert_eq!(r, reason);
        } else {
            panic!("Expected Disconnected variant");
        }
    }

    #[test]
    fn test_reconnected_accounts_count() {
        let event = AccountNotification::Reconnected {
            accounts_restored: 5,
        };

        if let AccountNotification::Reconnected { accounts_restored } = event {
            assert_eq!(accounts_restored, 5);
        } else {
            panic!("Expected Reconnected variant");
        }
    }

    #[test]
    fn test_connection_warning_message_preserved() {
        let msg = "Heartbeat response took 5000ms";
        let event = AccountNotification::ConnectionWarning {
            message: msg.to_string(),
        };

        if let AccountNotification::ConnectionWarning { message } = event {
            assert_eq!(message, msg);
        } else {
            panic!("Expected ConnectionWarning variant");
        }
    }

    // ===== ParseError and Unknown Event Tests =====

    #[test]
    fn test_parse_error_creation() {
        let event = AccountNotification::ParseError {
            action: "order".to_string(),
            error: "missing required field".to_string(),
            raw_data: serde_json::json!({"incomplete": "data"}),
        };

        assert!(event.is_parse_error());
        assert!(event.is_error_condition());
        assert!(!event.is_order());
        assert!(!event.is_healthy());
    }

    #[test]
    fn test_parse_error_preserves_data() {
        let raw = serde_json::json!({"id": 12345, "malformed": true});
        let event = AccountNotification::ParseError {
            action: "order".to_string(),
            error: "unexpected field type".to_string(),
            raw_data: raw.clone(),
        };

        if let AccountNotification::ParseError { action, error, raw_data } = event {
            assert_eq!(action, "order");
            assert!(error.contains("unexpected"));
            assert_eq!(raw_data, raw);
        } else {
            panic!("Expected ParseError variant");
        }
    }

    #[test]
    fn test_unknown_is_error_condition() {
        let event = AccountNotification::Unknown(serde_json::json!({"action": "new-api-feature"}));
        assert!(event.is_unknown());
        assert!(event.is_error_condition());
        assert!(!event.is_healthy());
    }

    #[test]
    fn test_is_error_condition_comprehensive() {
        // Error conditions
        assert!(AccountNotification::Disconnected { reason: "test".to_string() }.is_error_condition());
        assert!(AccountNotification::ConnectionWarning { message: "test".to_string() }.is_error_condition());
        assert!(AccountNotification::Unknown(serde_json::json!({})).is_error_condition());
        assert!(AccountNotification::ParseError {
            action: "order".to_string(),
            error: "test".to_string(),
            raw_data: serde_json::json!({}),
        }.is_error_condition());

        // Non-error conditions
        assert!(!AccountNotification::Heartbeat.is_error_condition());
        assert!(!AccountNotification::Order(OrderNotification::default()).is_error_condition());
        assert!(!AccountNotification::Position(PositionNotification::default()).is_error_condition());
        assert!(!AccountNotification::Balance(BalanceNotification::default()).is_error_condition());
        assert!(!AccountNotification::Reconnected { accounts_restored: 1 }.is_error_condition());
    }

    #[test]
    fn test_parse_error_for_all_action_types() {
        // Test that parse errors can be created for all notification types
        for action in &["order", "position", "balance", "quote-alert"] {
            let event = AccountNotification::ParseError {
                action: action.to_string(),
                error: "test error".to_string(),
                raw_data: serde_json::json!({}),
            };
            assert!(event.is_parse_error());
            if let AccountNotification::ParseError { action: a, .. } = event {
                assert_eq!(&a, action);
            }
        }
    }
}
