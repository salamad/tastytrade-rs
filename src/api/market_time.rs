//! Market time service for trading session information.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::client::ClientInner;
use crate::models::EquitySession;
use crate::Result;

/// Service for market time and session operations.
///
/// Provides information about trading sessions, market hours,
/// and market state (open, closed, pre-market, after-hours).
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Get current equity market session
/// let session = client.market_time().equities_session(None).await?;
/// println!("Market state: {:?}", session.state);
/// println!("Open at: {:?}", session.open_at);
/// println!("Close at: {:?}", session.close_at);
///
/// if session.is_open() {
///     println!("Market is currently open!");
/// }
/// # Ok(())
/// # }
/// ```
pub struct MarketTimeService {
    inner: Arc<ClientInner>,
}

impl MarketTimeService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get the current equity market session.
    ///
    /// Returns information about the current trading session including
    /// open/close times and market state.
    ///
    /// # Arguments
    ///
    /// * `current_time` - Optional time to base the session lookup on.
    ///   If not provided, uses the current time.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
    /// let session = client.market_time().equities_session(None).await?;
    ///
    /// if session.is_open() {
    ///     println!("Market is open until {:?}", session.close_at);
    /// } else if session.is_pre_market() {
    ///     println!("Pre-market session, opens at {:?}", session.open_at);
    /// } else {
    ///     println!("Market is closed");
    ///     if let Some(next) = &session.next_session {
    ///         println!("Next session opens at {:?}", next.open_at);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn equities_session(
        &self,
        current_time: Option<DateTime<Utc>>,
    ) -> Result<EquitySession> {
        #[derive(Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Query {
            #[serde(skip_serializing_if = "Option::is_none")]
            current_time: Option<String>,
        }

        let query = Query {
            current_time: current_time.map(|t| t.to_rfc3339()),
        };

        // Build the path with optional query parameter
        let path = if let Some(time) = &query.current_time {
            format!(
                "/market-time/equities/sessions/current?current-time={}",
                urlencoding::encode(time)
            )
        } else {
            "/market-time/equities/sessions/current".to_string()
        };

        self.inner.get(&path).await
    }

    /// Get the current equity market session using the current time.
    ///
    /// Convenience method that calls `equities_session(None)`.
    pub async fn current_equities_session(&self) -> Result<EquitySession> {
        self.equities_session(None).await
    }

    /// Check if the equity market is currently open.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
    /// if client.market_time().is_market_open().await? {
    ///     println!("Market is open for trading!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_market_open(&self) -> Result<bool> {
        let session = self.current_equities_session().await?;
        Ok(session.is_open())
    }
}
