//! Balance and position models.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::{InstrumentType, PriceEffect, QuantityDirection};

/// Account balance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountBalance {
    /// Account number
    pub account_number: String,
    /// Cash balance
    #[serde(default)]
    pub cash_balance: Option<Decimal>,
    /// Long stock/ETF value
    #[serde(default)]
    pub long_equity_value: Option<Decimal>,
    /// Short stock/ETF value
    #[serde(default)]
    pub short_equity_value: Option<Decimal>,
    /// Long derivative (options/futures) value
    #[serde(default)]
    pub long_derivative_value: Option<Decimal>,
    /// Short derivative value
    #[serde(default)]
    pub short_derivative_value: Option<Decimal>,
    /// Long futures value
    #[serde(default)]
    pub long_futures_value: Option<Decimal>,
    /// Short futures value
    #[serde(default)]
    pub short_futures_value: Option<Decimal>,
    /// Long futures derivative value
    #[serde(default)]
    pub long_futures_derivative_value: Option<Decimal>,
    /// Short futures derivative value
    #[serde(default)]
    pub short_futures_derivative_value: Option<Decimal>,
    /// Long marginable equity value
    #[serde(default)]
    pub long_marginable_value: Option<Decimal>,
    /// Short marginable equity value
    #[serde(default)]
    pub short_marginable_value: Option<Decimal>,
    /// Margin equity (total equity value)
    #[serde(default)]
    pub margin_equity: Option<Decimal>,
    /// Buying power for equities
    #[serde(default)]
    pub equity_buying_power: Option<Decimal>,
    /// Buying power for derivatives
    #[serde(default)]
    pub derivative_buying_power: Option<Decimal>,
    /// Day trading buying power
    #[serde(default)]
    pub day_trading_buying_power: Option<Decimal>,
    /// Futures margin requirement
    #[serde(default)]
    pub futures_margin_requirement: Option<Decimal>,
    /// Available trading funds
    #[serde(default)]
    pub available_trading_funds: Option<Decimal>,
    /// Maintenance requirement
    #[serde(default)]
    pub maintenance_requirement: Option<Decimal>,
    /// Maintenance call value
    #[serde(default)]
    pub maintenance_call_value: Option<Decimal>,
    /// Regulation T call value
    #[serde(default)]
    pub reg_t_call_value: Option<Decimal>,
    /// Day trade call value
    #[serde(default)]
    pub day_trade_call_value: Option<Decimal>,
    /// Day equity call value
    #[serde(default)]
    pub day_equity_call_value: Option<Decimal>,
    /// Net liquidating value (total account value)
    #[serde(default)]
    pub net_liquidating_value: Option<Decimal>,
    /// Cash available for withdrawal
    #[serde(default)]
    pub cash_available_to_withdraw: Option<Decimal>,
    /// Day trade excess
    #[serde(default)]
    pub day_trade_excess: Option<Decimal>,
    /// Pending cash
    #[serde(default)]
    pub pending_cash: Option<Decimal>,
    /// Pending cash effect
    #[serde(default)]
    pub pending_cash_effect: Option<PriceEffect>,
    /// Snapshot date
    #[serde(default)]
    pub snapshot_date: Option<NaiveDate>,
    /// Timestamp of balance update
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Historical balance snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BalanceSnapshot {
    /// Account number
    pub account_number: String,
    /// Snapshot date
    pub snapshot_date: NaiveDate,
    /// Cash balance at snapshot
    #[serde(default)]
    pub cash_balance: Option<Decimal>,
    /// Long equity value at snapshot
    #[serde(default)]
    pub long_equity_value: Option<Decimal>,
    /// Short equity value at snapshot
    #[serde(default)]
    pub short_equity_value: Option<Decimal>,
    /// Net liquidating value at snapshot
    #[serde(default)]
    pub net_liquidating_value: Option<Decimal>,
    /// Total equity at snapshot
    #[serde(default)]
    pub total_equity: Option<Decimal>,
}

/// Net liquidation history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NetLiqHistory {
    /// The date of the snapshot
    pub snapshot_date: NaiveDate,
    /// Open value for the day
    #[serde(default)]
    pub open: Option<Decimal>,
    /// High value for the day
    #[serde(default)]
    pub high: Option<Decimal>,
    /// Low value for the day
    #[serde(default)]
    pub low: Option<Decimal>,
    /// Close value for the day
    #[serde(default)]
    pub close: Option<Decimal>,
    /// Total cash at close
    #[serde(default)]
    pub total_cash: Option<Decimal>,
    /// Total equity at close
    #[serde(default)]
    pub total_equity: Option<Decimal>,
    /// Total close value
    #[serde(default)]
    pub total_close: Option<Decimal>,
    /// Pending cash
    #[serde(default)]
    pub pending_cash: Option<Decimal>,
}

/// Current position in an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Position {
    /// Account number
    pub account_number: String,
    /// Trading symbol
    pub symbol: String,
    /// Type of instrument
    pub instrument_type: InstrumentType,
    /// Underlying symbol (for derivatives)
    #[serde(default)]
    pub underlying_symbol: Option<String>,
    /// Position quantity
    pub quantity: Decimal,
    /// Direction of the position (long/short)
    pub quantity_direction: QuantityDirection,
    /// Previous day's closing price
    #[serde(default)]
    pub close_price: Option<Decimal>,
    /// Average cost basis per share/contract
    #[serde(default)]
    pub average_open_price: Option<Decimal>,
    /// Average cost per share with yield adjustments
    #[serde(default)]
    pub average_yearly_market_close_price: Option<Decimal>,
    /// Average daily price
    #[serde(default)]
    pub average_daily_market_close_price: Option<Decimal>,
    /// Contract multiplier (100 for standard options)
    #[serde(default)]
    pub multiplier: Option<i32>,
    /// Effect on cost (debit/credit)
    #[serde(default)]
    pub cost_effect: Option<PriceEffect>,
    /// Whether this is a closing position
    #[serde(default)]
    pub is_closing_only: bool,
    /// Whether this position is suppressed
    #[serde(default)]
    pub is_suppressed: bool,
    /// Realized day gain/loss
    #[serde(default)]
    pub realized_day_gain: Option<Decimal>,
    /// Realized day gain effect
    #[serde(default)]
    pub realized_day_gain_effect: Option<PriceEffect>,
    /// Realized day gain date
    #[serde(default)]
    pub realized_day_gain_date: Option<NaiveDate>,
    /// Realized gain/loss since opening
    #[serde(default)]
    pub realized_today: Option<Decimal>,
    /// Realized gain effect
    #[serde(default)]
    pub realized_today_effect: Option<PriceEffect>,
    /// Realized date
    #[serde(default)]
    pub realized_today_date: Option<NaiveDate>,
    /// Expiration date (for options/futures)
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// When the position was created
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    /// When the position was last updated
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl Position {
    /// Calculate the market value of the position.
    ///
    /// Returns `None` if close price is not available.
    pub fn market_value(&self) -> Option<Decimal> {
        self.close_price.map(|price| {
            let multiplier = Decimal::from(self.multiplier.unwrap_or(1));
            price * self.quantity * multiplier
        })
    }

    /// Calculate unrealized P&L for the position.
    ///
    /// Returns `None` if required prices are not available.
    pub fn unrealized_pnl(&self) -> Option<Decimal> {
        match (self.close_price, self.average_open_price) {
            (Some(close), Some(open)) => {
                let multiplier = Decimal::from(self.multiplier.unwrap_or(1));
                let pnl = (close - open) * self.quantity * multiplier;
                match self.quantity_direction {
                    QuantityDirection::Long => Some(pnl),
                    QuantityDirection::Short => Some(-pnl),
                }
            }
            _ => None,
        }
    }

    /// Returns `true` if this is a long position.
    pub fn is_long(&self) -> bool {
        matches!(self.quantity_direction, QuantityDirection::Long)
    }

    /// Returns `true` if this is a short position.
    pub fn is_short(&self) -> bool {
        matches!(self.quantity_direction, QuantityDirection::Short)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_market_value() {
        let pos = Position {
            account_number: "5WV12345".to_string(),
            symbol: "AAPL".to_string(),
            instrument_type: InstrumentType::Equity,
            underlying_symbol: None,
            quantity: dec!(100),
            quantity_direction: QuantityDirection::Long,
            close_price: Some(dec!(150.00)),
            average_open_price: Some(dec!(140.00)),
            average_yearly_market_close_price: None,
            average_daily_market_close_price: None,
            multiplier: Some(1),
            cost_effect: None,
            is_closing_only: false,
            is_suppressed: false,
            realized_day_gain: None,
            realized_day_gain_effect: None,
            realized_day_gain_date: None,
            realized_today: None,
            realized_today_effect: None,
            realized_today_date: None,
            expires_at: None,
            created_at: None,
            updated_at: None,
        };

        assert_eq!(pos.market_value(), Some(dec!(15000.00)));
        assert_eq!(pos.unrealized_pnl(), Some(dec!(1000.00)));
    }

    #[test]
    fn test_option_position_value() {
        let pos = Position {
            account_number: "5WV12345".to_string(),
            symbol: "AAPL  240119C00150000".to_string(),
            instrument_type: InstrumentType::EquityOption,
            underlying_symbol: Some("AAPL".to_string()),
            quantity: dec!(10),
            quantity_direction: QuantityDirection::Long,
            close_price: Some(dec!(5.50)),
            average_open_price: Some(dec!(3.00)),
            average_yearly_market_close_price: None,
            average_daily_market_close_price: None,
            multiplier: Some(100),
            cost_effect: None,
            is_closing_only: false,
            is_suppressed: false,
            realized_day_gain: None,
            realized_day_gain_effect: None,
            realized_day_gain_date: None,
            realized_today: None,
            realized_today_effect: None,
            realized_today_date: None,
            expires_at: None,
            created_at: None,
            updated_at: None,
        };

        // 10 contracts * $5.50 * 100 multiplier = $5,500
        assert_eq!(pos.market_value(), Some(dec!(5500.00)));
        // (5.50 - 3.00) * 10 * 100 = $2,500 profit
        assert_eq!(pos.unrealized_pnl(), Some(dec!(2500.00)));
    }
}
