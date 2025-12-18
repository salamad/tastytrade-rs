//! Order models for placing and managing trades.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::*;

/// A new order to be submitted.
///
/// Use [`NewOrderBuilder`] for a convenient way to construct orders.
///
/// # Example
///
/// ```
/// use tastytrade_rs::models::{
///     NewOrderBuilder, OrderType, TimeInForce, OrderAction, InstrumentType, OrderLeg, PriceEffect
/// };
/// use rust_decimal_macros::dec;
///
/// let order = NewOrderBuilder::default()
///     .time_in_force(TimeInForce::Day)
///     .order_type(OrderType::Limit)
///     .price(dec!(150.00))
///     .price_effect(PriceEffect::Debit)
///     .legs(vec![
///         OrderLeg {
///             instrument_type: InstrumentType::Equity,
///             symbol: "AAPL".to_string(),
///             quantity: dec!(10),
///             action: OrderAction::Buy,
///         }
///     ])
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NewOrder {
    /// How long the order remains active
    pub time_in_force: TimeInForce,
    /// Type of order (limit, market, etc.)
    pub order_type: OrderType,
    /// Limit price (required for limit orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
    /// Whether the price results in a credit or debit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_effect: Option<PriceEffect>,
    /// Stop trigger price (required for stop orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_trigger: Option<Decimal>,
    /// Date for GTD orders
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gtc_date: Option<NaiveDate>,
    /// Notional value (for notional orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,
    /// Effect of notional value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_effect: Option<PriceEffect>,
    /// Order legs (instruments to trade)
    pub legs: Vec<OrderLeg>,
    /// Order rules for conditional orders
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<OrderRules>,
    /// Client-provided identifier for idempotency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_identifier: Option<String>,
}

/// Builder for creating new orders with validation.
#[derive(Debug, Default, Clone)]
pub struct NewOrderBuilder {
    time_in_force: Option<TimeInForce>,
    order_type: Option<OrderType>,
    price: Option<Decimal>,
    price_effect: Option<PriceEffect>,
    stop_trigger: Option<Decimal>,
    gtc_date: Option<NaiveDate>,
    value: Option<Decimal>,
    value_effect: Option<PriceEffect>,
    legs: Vec<OrderLeg>,
    rules: Option<OrderRules>,
    external_identifier: Option<String>,
}

impl NewOrderBuilder {
    /// Create a new order builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the time in force.
    pub fn time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = Some(tif);
        self
    }

    /// Set the order type.
    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    /// Set the limit price.
    pub fn price(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    /// Set the price effect (credit/debit).
    pub fn price_effect(mut self, effect: PriceEffect) -> Self {
        self.price_effect = Some(effect);
        self
    }

    /// Set the stop trigger price.
    pub fn stop_trigger(mut self, price: Decimal) -> Self {
        self.stop_trigger = Some(price);
        self
    }

    /// Set the GTC date for GTD orders.
    pub fn gtc_date(mut self, date: NaiveDate) -> Self {
        self.gtc_date = Some(date);
        self
    }

    /// Set the notional value.
    pub fn value(mut self, value: Decimal) -> Self {
        self.value = Some(value);
        self
    }

    /// Set the value effect.
    pub fn value_effect(mut self, effect: PriceEffect) -> Self {
        self.value_effect = Some(effect);
        self
    }

    /// Set the order legs.
    pub fn legs(mut self, legs: Vec<OrderLeg>) -> Self {
        self.legs = legs;
        self
    }

    /// Add a single leg to the order.
    pub fn add_leg(mut self, leg: OrderLeg) -> Self {
        self.legs.push(leg);
        self
    }

    /// Add a leg with the specified parameters.
    ///
    /// This is a convenience method that creates an OrderLeg for you.
    pub fn leg(
        mut self,
        instrument_type: InstrumentType,
        symbol: impl Into<String>,
        action: OrderAction,
        quantity: impl Into<Decimal>,
    ) -> Self {
        self.legs.push(OrderLeg::new(
            instrument_type,
            symbol,
            quantity.into(),
            action,
        ));
        self
    }

    /// Set order rules for conditional orders.
    pub fn rules(mut self, rules: OrderRules) -> Self {
        self.rules = Some(rules);
        self
    }

    /// Set an external identifier for idempotency.
    pub fn external_identifier(mut self, id: impl Into<String>) -> Self {
        self.external_identifier = Some(id.into());
        self
    }

    /// Build the order, validating all fields.
    pub fn build(self) -> crate::Result<NewOrder> {
        let time_in_force = self.time_in_force.ok_or_else(|| {
            crate::Error::InvalidInput("time_in_force is required".to_string())
        })?;

        let order_type = self.order_type.ok_or_else(|| {
            crate::Error::InvalidInput("order_type is required".to_string())
        })?;

        if self.legs.is_empty() {
            return Err(crate::Error::InvalidInput(
                "Order must have at least one leg".to_string(),
            ));
        }

        // Validate price for limit orders
        if matches!(order_type, OrderType::Limit | OrderType::MarketableLimit | OrderType::StopLimit)
            && self.price.is_none()
        {
            return Err(crate::Error::InvalidInput(
                "Limit orders require a price".to_string(),
            ));
        }

        // Validate stop trigger for stop orders
        if matches!(order_type, OrderType::Stop | OrderType::StopLimit)
            && self.stop_trigger.is_none()
        {
            return Err(crate::Error::InvalidInput(
                "Stop orders require a stop_trigger price".to_string(),
            ));
        }

        // Validate GTC date for GTD orders
        if matches!(time_in_force, TimeInForce::Gtd) && self.gtc_date.is_none() {
            return Err(crate::Error::InvalidInput(
                "GTD orders require a gtc_date".to_string(),
            ));
        }

        // Validate price/value mutual exclusivity
        if self.price.is_some() && self.value.is_some() {
            return Err(crate::Error::InvalidInput(
                "Cannot specify both price and value".to_string(),
            ));
        }

        Ok(NewOrder {
            time_in_force,
            order_type,
            price: self.price,
            price_effect: self.price_effect,
            stop_trigger: self.stop_trigger,
            gtc_date: self.gtc_date,
            value: self.value,
            value_effect: self.value_effect,
            legs: self.legs,
            rules: self.rules,
            external_identifier: self.external_identifier,
        })
    }
}

/// A single leg of an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderLeg {
    /// Type of instrument
    pub instrument_type: InstrumentType,
    /// Trading symbol
    pub symbol: String,
    /// Quantity to trade
    pub quantity: Decimal,
    /// Action (buy/sell)
    pub action: OrderAction,
}

impl OrderLeg {
    /// Create a new order leg.
    pub fn new(
        instrument_type: InstrumentType,
        symbol: impl Into<String>,
        quantity: Decimal,
        action: OrderAction,
    ) -> Self {
        Self {
            instrument_type,
            symbol: symbol.into(),
            quantity,
            action,
        }
    }

    /// Create a leg to buy equity.
    pub fn buy_equity(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(InstrumentType::Equity, symbol, quantity, OrderAction::Buy)
    }

    /// Create a leg to sell equity.
    pub fn sell_equity(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(InstrumentType::Equity, symbol, quantity, OrderAction::Sell)
    }

    /// Create a leg to buy to open an option.
    pub fn buy_to_open_option(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(
            InstrumentType::EquityOption,
            symbol,
            quantity,
            OrderAction::BuyToOpen,
        )
    }

    /// Create a leg to sell to open an option.
    pub fn sell_to_open_option(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(
            InstrumentType::EquityOption,
            symbol,
            quantity,
            OrderAction::SellToOpen,
        )
    }

    /// Create a leg to buy to close an option.
    pub fn buy_to_close_option(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(
            InstrumentType::EquityOption,
            symbol,
            quantity,
            OrderAction::BuyToClose,
        )
    }

    /// Create a leg to sell to close an option.
    pub fn sell_to_close_option(symbol: impl Into<String>, quantity: Decimal) -> Self {
        Self::new(
            InstrumentType::EquityOption,
            symbol,
            quantity,
            OrderAction::SellToClose,
        )
    }
}

/// Rules for conditional orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderRules {
    /// Route after rule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_after: Option<RouteAfterRule>,
    /// Order conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<OrderCondition>>,
}

/// Rule for routing orders after market hours.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RouteAfterRule {
    /// Whether to route after market hours
    pub route_after_market_hours: bool,
    /// Whether to cancel at market close
    pub cancel_at_market_close: bool,
}

/// Condition for a conditional order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderCondition {
    /// Action to take when condition is met
    pub action: ConditionAction,
    /// Symbol to monitor
    pub symbol: String,
    /// Type of instrument
    pub instrument_type: InstrumentType,
    /// Price comparison operator
    pub price_comparison: PriceComparison,
    /// Price threshold
    pub price: Decimal,
}

/// Complex order (OCO/OTOCO).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NewComplexOrder {
    /// Type of complex order
    #[serde(rename = "type")]
    pub order_type: ComplexOrderType,
    /// Component orders
    pub orders: Vec<NewOrder>,
    /// Trigger order (for OTOCO)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_order: Option<NewOrder>,
}

/// A placed/existing order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Order {
    /// Order ID
    pub id: String,
    /// Account number
    pub account_number: String,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Order type
    pub order_type: OrderType,
    /// Underlying symbol
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// Underlying instrument type
    #[serde(default)]
    pub underlying_instrument_type: Option<InstrumentType>,
    /// Current status
    pub status: OrderStatus,
    /// Limit price
    #[serde(default)]
    pub price: Option<Decimal>,
    /// Price effect
    #[serde(default)]
    pub price_effect: Option<PriceEffect>,
    /// Stop trigger price
    #[serde(default)]
    pub stop_trigger: Option<Decimal>,
    /// Notional value
    #[serde(default)]
    pub value: Option<Decimal>,
    /// Value effect
    #[serde(default)]
    pub value_effect: Option<PriceEffect>,
    /// Order legs
    pub legs: Vec<FilledOrderLeg>,
    /// Total order size
    #[serde(default)]
    pub size: Option<Decimal>,
    /// Quantity filled
    #[serde(default)]
    pub filled_quantity: Option<Decimal>,
    /// Remaining quantity
    #[serde(default)]
    pub remaining_quantity: Option<Decimal>,
    /// Cancellable flag
    #[serde(default)]
    pub cancellable: bool,
    /// Editable flag
    #[serde(default)]
    pub editable: bool,
    /// Edited flag
    #[serde(default)]
    pub edited: bool,
    /// External identifier
    #[serde(default)]
    pub external_identifier: Option<String>,
    /// When the order was received
    #[serde(default)]
    pub received_at: Option<DateTime<Utc>>,
    /// When the order was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// When the order was filled/cancelled/expired
    #[serde(default)]
    pub terminal_at: Option<DateTime<Utc>>,
    /// GTC date
    #[serde(default)]
    pub gtc_date: Option<NaiveDate>,
    /// Complex order ID (if part of a complex order)
    #[serde(default)]
    pub complex_order_id: Option<String>,
    /// Complex order tag
    #[serde(default)]
    pub complex_order_tag: Option<String>,
    /// Rejection reason
    #[serde(default)]
    pub reject_reason: Option<String>,
    /// Cancellation reason
    #[serde(default)]
    pub cancel_reason: Option<String>,
}

impl Order {
    /// Get the order ID as a strongly-typed value.
    pub fn order_id(&self) -> super::OrderId {
        super::OrderId::new(&self.id)
    }

    /// Returns `true` if the order can be cancelled.
    pub fn is_cancellable(&self) -> bool {
        self.cancellable && !self.status.is_terminal()
    }

    /// Returns `true` if the order can be modified.
    pub fn is_editable(&self) -> bool {
        self.editable && !self.status.is_terminal()
    }

    /// Returns `true` if the order is completely filled.
    pub fn is_filled(&self) -> bool {
        matches!(self.status, OrderStatus::Filled)
    }

    /// Returns `true` if the order is still working.
    pub fn is_working(&self) -> bool {
        self.status.is_working()
    }

    /// Calculate the fill percentage.
    pub fn fill_percentage(&self) -> Option<Decimal> {
        match (self.filled_quantity, self.size) {
            (Some(filled), Some(size)) if size > Decimal::ZERO => {
                Some((filled / size) * Decimal::from(100))
            }
            _ => None,
        }
    }
}

/// A leg of a filled order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FilledOrderLeg {
    /// Type of instrument
    pub instrument_type: InstrumentType,
    /// Trading symbol
    pub symbol: String,
    /// Quantity ordered
    #[serde(default)]
    pub quantity: Option<Decimal>,
    /// Remaining quantity
    #[serde(default)]
    pub remaining_quantity: Option<Decimal>,
    /// Action
    pub action: OrderAction,
    /// Fill prices
    #[serde(default)]
    pub fills: Vec<Fill>,
}

/// Fill information for an order leg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Fill {
    /// Quantity filled
    pub quantity: Decimal,
    /// Fill price
    pub fill_price: Decimal,
    /// When the fill occurred
    pub filled_at: DateTime<Utc>,
    /// Destination exchange
    #[serde(default)]
    pub destination_venue: Option<String>,
    /// External execution ID
    #[serde(default)]
    pub ext_exec_id: Option<String>,
    /// External group fill ID
    #[serde(default)]
    pub ext_group_fill_id: Option<String>,
}

impl Fill {
    /// Calculate the total value of this fill.
    pub fn value(&self, multiplier: i32) -> Decimal {
        self.quantity * self.fill_price * Decimal::from(multiplier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_builder_valid() {
        let order = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(dec!(150.00))
            .price_effect(PriceEffect::Debit)
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(10)))
            .build()
            .unwrap();

        assert_eq!(order.time_in_force, TimeInForce::Day);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.price, Some(dec!(150.00)));
        assert_eq!(order.legs.len(), 1);
    }

    #[test]
    fn test_order_builder_missing_tif() {
        let result = NewOrderBuilder::new()
            .order_type(OrderType::Limit)
            .price(dec!(150.00))
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(10)))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_order_builder_limit_no_price() {
        let result = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(10)))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_order_builder_stop_no_trigger() {
        let result = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Stop)
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(10)))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_order_leg_helpers() {
        let buy = OrderLeg::buy_equity("AAPL", dec!(100));
        assert_eq!(buy.instrument_type, InstrumentType::Equity);
        assert_eq!(buy.action, OrderAction::Buy);

        let sell = OrderLeg::sell_to_close_option("AAPL  240119C00150000", dec!(5));
        assert_eq!(sell.instrument_type, InstrumentType::EquityOption);
        assert_eq!(sell.action, OrderAction::SellToClose);
    }
}
