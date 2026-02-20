//! Enumeration types for the TastyTrade API.
//!
//! This module contains all the enum types used throughout the API,
//! including order types, instrument types, status codes, and more.

use serde::{Deserialize, Serialize};

/// Type of financial instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum InstrumentType {
    /// Common stock or ETF
    #[serde(rename = "Equity")]
    #[default]
    Equity,
    /// Stock option contract
    #[serde(rename = "Equity Option")]
    EquityOption,
    /// Futures contract
    #[serde(rename = "Future")]
    Future,
    /// Option on a futures contract
    #[serde(rename = "Future Option")]
    FutureOption,
    /// Cryptocurrency
    #[serde(rename = "Cryptocurrency")]
    Cryptocurrency,
    /// Warrant
    #[serde(rename = "Warrant")]
    Warrant,
    /// Index
    #[serde(rename = "Index")]
    Index,
    /// Unknown instrument type
    #[serde(other)]
    Unknown,
}

impl InstrumentType {
    /// Returns `true` if this instrument type is a derivative.
    pub fn is_derivative(&self) -> bool {
        matches!(
            self,
            InstrumentType::EquityOption
                | InstrumentType::Future
                | InstrumentType::FutureOption
        )
    }
}

/// Order type specifying how the order should be executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    /// Limit order - execute at specified price or better
    Limit,
    /// Market order - execute immediately at current market price
    Market,
    /// Marketable limit - limit order priced to execute immediately
    #[serde(rename = "Marketable Limit")]
    MarketableLimit,
    /// Stop order - becomes market order when stop price is reached
    Stop,
    /// Stop limit - becomes limit order when stop price is reached
    #[serde(rename = "Stop Limit")]
    StopLimit,
    /// Notional market order - specify dollar amount instead of shares
    #[serde(rename = "Notional Market")]
    NotionalMarket,
}

/// Action to take for an order leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderAction {
    /// Buy to open a new long position (options/futures)
    #[serde(rename = "Buy to Open")]
    BuyToOpen,
    /// Buy to close an existing short position (options/futures)
    #[serde(rename = "Buy to Close")]
    BuyToClose,
    /// Sell to open a new short position (options/futures)
    #[serde(rename = "Sell to Open")]
    SellToOpen,
    /// Sell to close an existing long position (options/futures)
    #[serde(rename = "Sell to Close")]
    SellToClose,
    /// Buy (for equities)
    Buy,
    /// Sell (for equities)
    Sell,
}

impl OrderAction {
    /// Returns `true` if this is a buy action.
    pub fn is_buy(&self) -> bool {
        matches!(
            self,
            OrderAction::BuyToOpen | OrderAction::BuyToClose | OrderAction::Buy
        )
    }

    /// Returns `true` if this is a sell action.
    pub fn is_sell(&self) -> bool {
        !self.is_buy()
    }

    /// Returns `true` if this opens a new position.
    pub fn is_opening(&self) -> bool {
        matches!(
            self,
            OrderAction::BuyToOpen | OrderAction::SellToOpen | OrderAction::Buy
        )
    }
}

/// Time in force specification for orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Day order - expires at end of trading day
    Day,
    /// Good till cancelled - remains active until filled or cancelled
    #[serde(rename = "GTC")]
    Gtc,
    /// Good till date - remains active until specified date
    #[serde(rename = "GTD")]
    Gtd,
    /// Extended hours - can execute during pre/post market
    Ext,
    /// Immediate or cancel - fill immediately or cancel
    #[serde(rename = "IOC")]
    Ioc,
}

/// Current status of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Order received by the system
    Received,
    /// Order routed to exchange
    Routed,
    /// Order is being processed
    #[serde(rename = "In Flight")]
    InFlight,
    /// Order is live and working
    Live,
    /// Cancel has been requested
    #[serde(rename = "Cancel Requested")]
    CancelRequested,
    /// Replace/modify has been requested
    #[serde(rename = "Replace Requested")]
    ReplaceRequested,
    /// Contingent order waiting for trigger
    Contingent,
    /// Order completely filled
    Filled,
    /// Order cancelled
    Cancelled,
    /// Order expired
    Expired,
    /// Order rejected
    Rejected,
    /// Order removed
    Removed,
    /// Order partially filled then removed
    #[serde(rename = "Partially Removed")]
    PartiallyRemoved,
}

impl OrderStatus {
    /// Returns `true` if the order is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderStatus::Filled
                | OrderStatus::Cancelled
                | OrderStatus::Expired
                | OrderStatus::Rejected
                | OrderStatus::Removed
                | OrderStatus::PartiallyRemoved
        )
    }

    /// Returns `true` if the order is still working.
    pub fn is_working(&self) -> bool {
        matches!(
            self,
            OrderStatus::Received
                | OrderStatus::Routed
                | OrderStatus::InFlight
                | OrderStatus::Live
                | OrderStatus::Contingent
        )
    }
}

/// Effect on price/value (credit or debit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceEffect {
    /// Credit - money received
    Credit,
    /// Debit - money paid
    Debit,
    /// No effect
    None,
    /// Unknown effect (forward-compatibility)
    #[serde(other)]
    Unknown,
}

/// Direction of a position quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuantityDirection {
    /// Long position
    Long,
    /// Short position
    Short,
    /// Zero quantity (fully closed/assigned positions)
    Zero,
    /// Unknown direction (forward-compatibility)
    #[serde(other)]
    Unknown,
}

/// Account margin type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarginOrCash {
    /// Margin account
    Margin,
    /// Cash account
    Cash,
    /// IRA Margin account
    #[serde(rename = "IRA Margin")]
    IraMargin,
}

/// Complex order type (multi-leg strategies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComplexOrderType {
    /// One-cancels-other
    #[serde(rename = "OCO")]
    Oco,
    /// One-triggers-one-cancels-other
    #[serde(rename = "OTOCO")]
    Otoco,
}

/// Option type (call or put).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OptionType {
    /// Call option
    #[serde(rename = "C")]
    Call,
    /// Put option
    #[serde(rename = "P")]
    Put,
}

impl OptionType {
    /// Returns `true` if this is a call option.
    pub fn is_call(&self) -> bool {
        matches!(self, OptionType::Call)
    }

    /// Returns `true` if this is a put option.
    pub fn is_put(&self) -> bool {
        matches!(self, OptionType::Put)
    }
}

/// Authority level for account access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityLevel {
    /// Full account owner access
    Owner,
    /// Trading permissions only
    TradeOnly,
    /// Read-only access
    ReadOnly,
}

/// Condition action for conditional orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionAction {
    /// Route the order when condition is met
    Route,
    /// Cancel the order when condition is met
    Cancel,
}

/// Price comparison operator for conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceComparison {
    /// Greater than or equal
    #[serde(rename = "GTE")]
    Gte,
    /// Less than or equal
    #[serde(rename = "LTE")]
    Lte,
}

/// Transaction type for account transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TransactionType {
    /// Trade execution
    #[default]
    Trade,
    /// Option expiration
    #[serde(rename = "Receive Deliver")]
    ReceiveDeliver,
    /// Dividend payment
    Dividend,
    /// Interest payment
    Interest,
    /// Money movement (deposit/withdrawal)
    #[serde(rename = "Money Movement")]
    MoneyMovement,
    /// Fee charged
    Fee,
    /// Credit or debit
    #[serde(rename = "Credit/Debit")]
    CreditDebit,
    /// Balance adjustment
    #[serde(rename = "Balance Adjustment")]
    BalanceAdjustment,
    /// Other transaction type
    #[serde(other)]
    Other,
}

/// Market session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketSession {
    /// Regular trading hours
    Regular,
    /// Extended hours (pre/post market)
    Extended,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instrument_type_derivative() {
        assert!(!InstrumentType::Equity.is_derivative());
        assert!(InstrumentType::EquityOption.is_derivative());
        assert!(InstrumentType::Future.is_derivative());
        assert!(InstrumentType::FutureOption.is_derivative());
        assert!(!InstrumentType::Cryptocurrency.is_derivative());
    }

    #[test]
    fn test_order_status_terminal() {
        assert!(OrderStatus::Filled.is_terminal());
        assert!(OrderStatus::Cancelled.is_terminal());
        assert!(!OrderStatus::Live.is_terminal());
        assert!(!OrderStatus::Received.is_terminal());
    }

    #[test]
    fn test_order_action_buy_sell() {
        assert!(OrderAction::BuyToOpen.is_buy());
        assert!(OrderAction::Buy.is_buy());
        assert!(!OrderAction::SellToClose.is_buy());
        assert!(OrderAction::SellToOpen.is_sell());
    }

    #[test]
    fn test_serde_roundtrip() {
        let order_type = OrderType::MarketableLimit;
        let json = serde_json::to_string(&order_type).unwrap();
        assert_eq!(json, "\"Marketable Limit\"");

        let parsed: OrderType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, order_type);
    }
}
