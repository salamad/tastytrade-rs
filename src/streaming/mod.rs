//! Real-time streaming services for market data and account updates.
//!
//! This module provides two streaming services:
//!
//! - **DXLink** - Real-time market data (quotes, greeks, trades, etc.)
//! - **Account Streamer** - Real-time account updates (orders, positions, balances)
//!
//! # DXLink Market Data
//!
//! ```no_run
//! use tastytrade_rs::streaming::{DxLinkStreamer, Quote};
//!
//! # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
//! // Get a DXLink streamer
//! let mut streamer = client.streaming().dxlink().await?;
//!
//! // Subscribe to quotes
//! streamer.subscribe::<Quote>(&["AAPL", "SPY"]).await?;
//!
//! // Process events
//! while let Some(event) = streamer.next().await {
//!     println!("{:?}", event?);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Account Streamer
//!
//! ```no_run
//! use tastytrade_rs::streaming::AccountStreamer;
//! use tastytrade_rs::AccountNumber;
//!
//! # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
//! // Get an account streamer
//! let mut streamer = client.streaming().account().await?;
//!
//! // Subscribe to account updates
//! let account = AccountNumber::new("5WV12345");
//! streamer.subscribe(&account).await?;
//!
//! // Process notifications
//! while let Some(notification) = streamer.next().await {
//!     println!("{:?}", notification?);
//! }
//! # Ok(())
//! # }
//! ```

pub mod dxlink;
pub mod account;

pub use dxlink::{DxLinkStreamer, DxEvent, DxEventTrait, EventType, Quote, Trade, Greeks, Summary, Profile, Candle, TheoPrice};
pub use account::{AccountStreamer, AccountNotification, OrderNotification, PositionNotification, BalanceNotification, QuoteAlertNotification};

use std::sync::Arc;
use crate::client::ClientInner;
use crate::Result;

/// Access point for streaming services.
pub struct StreamingServices {
    inner: Arc<ClientInner>,
}

impl StreamingServices {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Create a DXLink market data streamer.
    pub async fn dxlink(&self) -> Result<DxLinkStreamer> {
        DxLinkStreamer::connect(self.inner.clone()).await
    }

    /// Create an account activity streamer.
    pub async fn account(&self) -> Result<AccountStreamer> {
        AccountStreamer::connect(self.inner.clone()).await
    }
}
