//! Balances and positions service.

use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;

use crate::client::ClientInner;
use crate::models::{AccountBalance, AccountNumber, BalanceSnapshot, NetLiqHistory, Position};
use crate::Result;

/// Service for balance and position operations.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// use tastytrade_rs::AccountNumber;
///
/// let account = AccountNumber::new("5WV12345");
///
/// // Get current balances
/// let balance = client.balances().get(&account).await?;
/// println!("Net Liq: {:?}", balance.net_liquidating_value);
///
/// // Get positions
/// let positions = client.balances().positions(&account).await?;
/// for pos in positions {
///     println!("{}: {:?} shares", pos.symbol, pos.quantity);
/// }
/// # Ok(())
/// # }
/// ```
pub struct BalancesService {
    inner: Arc<ClientInner>,
}

impl BalancesService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get current balances for an account.
    pub async fn get(&self, account_number: &AccountNumber) -> Result<AccountBalance> {
        self.inner
            .get(&format!("/accounts/{}/balances", account_number))
            .await
    }

    /// Get balance snapshots (historical balances).
    ///
    /// # Arguments
    ///
    /// * `account_number` - The account to query
    /// * `start_date` - Optional start date for the range
    /// * `end_date` - Optional end date for the range
    ///
    /// Note: Uses End-of-Day (EOD) as the default time-of-day for snapshots.
    pub async fn snapshots(
        &self,
        account_number: &AccountNumber,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<BalanceSnapshot>> {
        #[derive(Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Query {
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_date: Option<String>,
            time_of_day: &'static str,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<BalanceSnapshot>,
        }

        // The API requires a snapshot-date, not date ranges
        // If dates are provided, use start_date; otherwise no date filter
        let snapshot_date = start_date.map(|d| d.format("%Y-%m-%d").to_string());

        let query = Query {
            snapshot_date,
            time_of_day: "EOD", // End of Day
        };

        let response: Response = self
            .inner
            .get_with_query(
                &format!("/accounts/{}/balance-snapshots", account_number),
                &query,
            )
            .await?;
        Ok(response.items)
    }

    /// Get current positions for an account.
    pub async fn positions(&self, account_number: &AccountNumber) -> Result<Vec<Position>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Position>,
        }

        let response: Response = self
            .inner
            .get(&format!("/accounts/{}/positions", account_number))
            .await?;
        Ok(response.items)
    }

    /// Get net liquidation history for an account.
    ///
    /// # Arguments
    ///
    /// * `account_number` - The account to query
    /// * `time_back` - How far back to look (e.g., "1d", "1m", "1y", "all")
    pub async fn net_liq_history(
        &self,
        account_number: &AccountNumber,
        time_back: Option<&str>,
    ) -> Result<Vec<NetLiqHistory>> {
        #[derive(Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            time_back: Option<&'a str>,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<NetLiqHistory>,
        }

        let query = Query { time_back };
        let response: Response = self
            .inner
            .get_with_query(
                &format!("/accounts/{}/net-liq-history", account_number),
                &query,
            )
            .await?;
        Ok(response.items)
    }
}
