//! Trading-specific models (fees, buying power, transactions).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};

use super::enums::{InstrumentType, PriceEffect, TransactionType};

/// Helper to deserialize IDs that may come as integers or strings.
fn deserialize_id<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IdValue {
        Int(i64),
        String(String),
        Null,
    }

    match IdValue::deserialize(deserializer)? {
        IdValue::Int(i) => Ok(Some(i)),
        IdValue::String(s) => s.parse::<i64>().map(Some).map_err(D::Error::custom),
        IdValue::Null => Ok(None),
    }
}

/// Effect on buying power from an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuyingPowerEffect {
    /// Change in margin requirement
    #[serde(default)]
    pub change_in_margin_requirement: Option<Decimal>,
    /// Effect of margin change (debit increases, credit decreases)
    #[serde(default)]
    pub change_in_margin_requirement_effect: Option<PriceEffect>,
    /// Change in buying power
    #[serde(default)]
    pub change_in_buying_power: Option<Decimal>,
    /// Effect of buying power change
    #[serde(default)]
    pub change_in_buying_power_effect: Option<PriceEffect>,
    /// Current buying power before order
    #[serde(default)]
    pub current_buying_power: Option<Decimal>,
    /// Effect on current buying power
    #[serde(default)]
    pub current_buying_power_effect: Option<PriceEffect>,
    /// New buying power after order
    #[serde(default)]
    pub new_buying_power: Option<Decimal>,
    /// Effect on new buying power
    #[serde(default)]
    pub new_buying_power_effect: Option<PriceEffect>,
    /// Margin requirement for this order in isolation
    #[serde(default)]
    pub isolated_order_margin_requirement: Option<Decimal>,
    /// Effect of isolated margin
    #[serde(default)]
    pub isolated_order_margin_requirement_effect: Option<PriceEffect>,
    /// Whether this is a spread trade
    #[serde(default)]
    pub is_spread: bool,
    /// Overall impact amount
    #[serde(default)]
    pub impact: Option<Decimal>,
    /// Overall impact effect
    #[serde(default)]
    pub effect: Option<PriceEffect>,
}

impl BuyingPowerEffect {
    /// Check if there's sufficient buying power for the order.
    pub fn has_sufficient_buying_power(&self) -> bool {
        match (self.new_buying_power, self.new_buying_power_effect) {
            (Some(bp), Some(effect)) => {
                bp > Decimal::ZERO || matches!(effect, PriceEffect::Credit)
            }
            _ => true, // Assume OK if we don't have the data
        }
    }
}

/// Fee calculation for an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FeeCalculation {
    /// Regulatory fees (SEC, FINRA, etc.)
    #[serde(default)]
    pub regulatory_fees: Option<Decimal>,
    /// Effect of regulatory fees
    #[serde(default)]
    pub regulatory_fees_effect: Option<PriceEffect>,
    /// Clearing fees
    #[serde(default)]
    pub clearing_fees: Option<Decimal>,
    /// Effect of clearing fees
    #[serde(default)]
    pub clearing_fees_effect: Option<PriceEffect>,
    /// Commission
    #[serde(default)]
    pub commission: Option<Decimal>,
    /// Effect of commission
    #[serde(default)]
    pub commission_effect: Option<PriceEffect>,
    /// Proprietary index option fees
    #[serde(default)]
    pub proprietary_index_option_fees: Option<Decimal>,
    /// Effect of index option fees
    #[serde(default)]
    pub proprietary_index_option_fees_effect: Option<PriceEffect>,
    /// Total fees
    #[serde(default)]
    pub total_fees: Option<Decimal>,
    /// Effect of total fees
    #[serde(default)]
    pub total_fees_effect: Option<PriceEffect>,
}

impl FeeCalculation {
    /// Get the total fees as a positive number.
    pub fn total_fees_amount(&self) -> Decimal {
        self.total_fees.unwrap_or(Decimal::ZERO).abs()
    }
}

/// Message from order validation or placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrderMessage {
    /// Message code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Preflight ID (for warnings that can be acknowledged)
    #[serde(default)]
    pub preflight_id: Option<String>,
}

impl OrderMessage {
    /// Check if this is a warning (can be acknowledged).
    pub fn is_warning(&self) -> bool {
        self.preflight_id.is_some()
    }
}

/// Response from placing an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlacedOrderResponse {
    /// The placed order
    pub order: super::Order,
    /// Effect on buying power
    #[serde(default)]
    pub buying_power_effect: Option<BuyingPowerEffect>,
    /// Fee calculation
    #[serde(default)]
    pub fee_calculation: Option<FeeCalculation>,
    /// Warnings (can be acknowledged)
    #[serde(default)]
    pub warnings: Option<Vec<OrderMessage>>,
    /// Errors
    #[serde(default)]
    pub errors: Option<Vec<OrderMessage>>,
}

impl PlacedOrderResponse {
    /// Check if there are any warnings.
    pub fn has_warnings(&self) -> bool {
        self.warnings.as_ref().map(|w| !w.is_empty()).unwrap_or(false)
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.errors.as_ref().map(|e| !e.is_empty()).unwrap_or(false)
    }
}

/// Response from dry-run order validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DryRunResponse {
    /// The order as it would be placed
    pub order: super::Order,
    /// Effect on buying power
    #[serde(default)]
    pub buying_power_effect: Option<BuyingPowerEffect>,
    /// Fee calculation
    #[serde(default)]
    pub fee_calculation: Option<FeeCalculation>,
    /// Warnings
    #[serde(default)]
    pub warnings: Option<Vec<OrderMessage>>,
    /// Errors (order would be rejected)
    #[serde(default)]
    pub errors: Option<Vec<OrderMessage>>,
}

impl DryRunResponse {
    /// Check if the order would be accepted.
    pub fn is_valid(&self) -> bool {
        !self.has_errors()
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.errors.as_ref().map(|e| !e.is_empty()).unwrap_or(false)
    }
}

/// Account transaction.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Transaction {
    /// Transaction ID (can be integer or string from API)
    #[serde(deserialize_with = "deserialize_id")]
    #[serde(default)]
    pub id: Option<i64>,
    /// Account number
    #[serde(default)]
    pub account_number: String,
    /// Transaction type
    #[serde(default)]
    pub transaction_type: Option<TransactionType>,
    /// Transaction sub-type
    #[serde(default)]
    pub transaction_sub_type: Option<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Symbol (for trades)
    #[serde(default)]
    pub symbol: Option<String>,
    /// Underlying symbol
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// Instrument type
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
    /// Action (buy/sell)
    #[serde(default)]
    pub action: Option<String>,
    /// Quantity
    #[serde(default)]
    pub quantity: Option<Decimal>,
    /// Price
    #[serde(default)]
    pub price: Option<Decimal>,
    /// Net value of transaction
    #[serde(default)]
    pub value: Option<Decimal>,
    /// Value effect
    #[serde(default)]
    pub value_effect: Option<PriceEffect>,
    /// Regulatory fees
    #[serde(default)]
    pub regulatory_fees: Option<Decimal>,
    /// Regulatory fees effect
    #[serde(default)]
    pub regulatory_fees_effect: Option<PriceEffect>,
    /// Clearing fees
    #[serde(default)]
    pub clearing_fees: Option<Decimal>,
    /// Clearing fees effect
    #[serde(default)]
    pub clearing_fees_effect: Option<PriceEffect>,
    /// Commission
    #[serde(default)]
    pub commission: Option<Decimal>,
    /// Commission effect
    #[serde(default)]
    pub commission_effect: Option<PriceEffect>,
    /// Order ID
    #[serde(default)]
    pub order_id: Option<String>,
    /// Execution ID
    #[serde(default)]
    pub exec_id: Option<String>,
    /// External execution ID
    #[serde(default)]
    pub ext_exec_id: Option<String>,
    /// External group ID
    #[serde(default)]
    pub ext_group_id: Option<String>,
    /// Execution date
    #[serde(default)]
    pub executed_at: Option<DateTime<Utc>>,
    /// Transaction date
    #[serde(default)]
    pub transaction_date: Option<NaiveDate>,
}

impl Transaction {
    /// Get the net amount of the transaction (value minus fees).
    pub fn net_amount(&self) -> Decimal {
        let value = self.value.unwrap_or(Decimal::ZERO);
        let reg_fees = self.regulatory_fees.unwrap_or(Decimal::ZERO);
        let clearing_fees = self.clearing_fees.unwrap_or(Decimal::ZERO);
        let commission = self.commission.unwrap_or(Decimal::ZERO);

        // Fees are typically debits, so subtract them
        value - reg_fees - clearing_fees - commission
    }
}

/// Watchlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Watchlist {
    /// Watchlist name
    pub name: String,
    /// Watchlist entries
    #[serde(default)]
    pub watchlist_entries: Vec<WatchlistEntry>,
    /// Group name
    #[serde(default)]
    pub group_name: Option<String>,
    /// Order index for sorting
    #[serde(default)]
    pub order_index: Option<i32>,
}

/// Entry in a watchlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WatchlistEntry {
    /// Trading symbol
    pub symbol: String,
    /// Instrument type
    #[serde(default)]
    pub instrument_type: Option<InstrumentType>,
}

impl WatchlistEntry {
    /// Create a new watchlist entry.
    pub fn new(symbol: impl Into<String>, instrument_type: Option<InstrumentType>) -> Self {
        Self {
            symbol: symbol.into(),
            instrument_type,
        }
    }

    /// Create an equity watchlist entry.
    pub fn equity(symbol: impl Into<String>) -> Self {
        Self::new(symbol, Some(InstrumentType::Equity))
    }

    /// Create an option watchlist entry.
    pub fn option(symbol: impl Into<String>) -> Self {
        Self::new(symbol, Some(InstrumentType::EquityOption))
    }

    /// Create a future watchlist entry.
    pub fn future(symbol: impl Into<String>) -> Self {
        Self::new(symbol, Some(InstrumentType::Future))
    }

    /// Create a cryptocurrency watchlist entry.
    pub fn crypto(symbol: impl Into<String>) -> Self {
        Self::new(symbol, Some(InstrumentType::Cryptocurrency))
    }
}

/// Public watchlist (tastytrade curated).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PublicWatchlist {
    /// Watchlist name
    pub name: String,
    /// Symbols in the watchlist
    #[serde(default)]
    pub symbols: Vec<String>,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_buying_power_effect() {
        let effect = BuyingPowerEffect {
            change_in_margin_requirement: Some(dec!(500)),
            change_in_margin_requirement_effect: Some(PriceEffect::Debit),
            change_in_buying_power: Some(dec!(-500)),
            change_in_buying_power_effect: Some(PriceEffect::Debit),
            current_buying_power: Some(dec!(10000)),
            current_buying_power_effect: Some(PriceEffect::Credit),
            new_buying_power: Some(dec!(9500)),
            new_buying_power_effect: Some(PriceEffect::Credit),
            isolated_order_margin_requirement: None,
            isolated_order_margin_requirement_effect: None,
            is_spread: false,
            impact: Some(dec!(500)),
            effect: Some(PriceEffect::Debit),
        };

        assert!(effect.has_sufficient_buying_power());
    }

    #[test]
    fn test_fee_calculation() {
        let fees = FeeCalculation {
            regulatory_fees: Some(dec!(0.02)),
            regulatory_fees_effect: Some(PriceEffect::Debit),
            clearing_fees: Some(dec!(0.05)),
            clearing_fees_effect: Some(PriceEffect::Debit),
            commission: Some(dec!(0)),
            commission_effect: Some(PriceEffect::None),
            proprietary_index_option_fees: Some(dec!(0)),
            proprietary_index_option_fees_effect: Some(PriceEffect::None),
            total_fees: Some(dec!(0.07)),
            total_fees_effect: Some(PriceEffect::Debit),
        };

        assert_eq!(fees.total_fees_amount(), dec!(0.07));
    }
}
