//! # tastytrade-rs
//!
//! A production-grade Rust client for the TastyTrade brokerage API.
//!
//! This crate provides comprehensive access to TastyTrade's trading platform,
//! including account management, order execution, market data, and real-time
//! streaming via WebSocket.
//!
//! ## Features
//!
//! - **Authentication**: OAuth2 and legacy session-based authentication
//! - **Account Management**: View accounts, balances, positions, and history
//! - **Order Management**: Place, modify, and cancel orders with dry-run support
//! - **Market Data**: Quotes, option chains, and market metrics
//! - **Real-time Streaming**: DXLink for market data, Account Streamer for notifications
//! - **Type Safety**: Strongly-typed models with compile-time guarantees
//! - **Async-first**: Built on Tokio for high-performance async I/O
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
//!
//! #[tokio::main]
//! async fn main() -> tastytrade_rs::Result<()> {
//!     // Create a client with credentials
//!     let client = TastytradeClient::login(
//!         "username",
//!         "password",
//!         Environment::Sandbox,
//!     ).await?;
//!
//!     // Get accounts
//!     let accounts = client.accounts().list().await?;
//!     println!("Found {} accounts", accounts.len());
//!
//!     // Get account balance
//!     if let Some(item) = accounts.first() {
//!         let account_number = AccountNumber::new(&item.account.account_number);
//!         let balance = client.balances()
//!             .get(&account_number)
//!             .await?;
//!         println!("Net liquidating value: {:?}", balance.net_liquidating_value);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Streaming Example
//!
//! ```rust,no_run
//! use tastytrade_rs::{TastytradeClient, Environment};
//! use tastytrade_rs::streaming::{Quote, DxEvent};
//!
//! #[tokio::main]
//! async fn main() -> tastytrade_rs::Result<()> {
//!     let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
//!
//!     // Create market data streamer
//!     let mut streamer = client.streaming().dxlink().await?;
//!
//!     // Subscribe to quotes
//!     streamer.subscribe::<Quote>(&["AAPL", "SPY", "QQQ"]).await?;
//!
//!     // Process incoming events
//!     while let Some(event) = streamer.next().await {
//!         match event? {
//!             DxEvent::Quote(quote) => {
//!                 println!("{}: bid={:?} ask={:?}",
//!                     quote.event_symbol,
//!                     quote.bid_price,
//!                     quote.ask_price
//!                 );
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Order Placement
//!
//! ```rust,no_run
//! use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
//! use tastytrade_rs::models::{NewOrderBuilder, OrderType, TimeInForce, OrderAction, InstrumentType};
//! use rust_decimal_macros::dec;
//!
//! #[tokio::main]
//! async fn main() -> tastytrade_rs::Result<()> {
//!     let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
//!     let account = AccountNumber::new("5WV12345");
//!
//!     // Build a limit order
//!     let order = NewOrderBuilder::new()
//!         .time_in_force(TimeInForce::Day)
//!         .order_type(OrderType::Limit)
//!         .price(dec!(150.00))
//!         .leg(InstrumentType::Equity, "AAPL", OrderAction::BuyToOpen, dec!(10))
//!         .build()?;
//!
//!     // Dry run first to check buying power
//!     let dry_run = client.orders().dry_run(&account, order.clone()).await?;
//!     println!("Buying power effect: {:?}", dry_run.buying_power_effect);
//!
//!     // Place the order
//!     let response = client.orders().place(&account, order).await?;
//!     println!("Order placed: {:?}", response.order.id);
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![deny(unsafe_code)]

pub mod api;
pub mod auth;
pub mod client;
pub mod error;
pub mod models;
pub mod streaming;

// Re-export primary types at crate root for convenience
pub use error::{Error, Result};
pub use models::{
    AccountNumber, OrderId, Symbol, StreamerSymbol,
    Environment, ApiVersion,
};
pub use client::{TastytradeClient, ClientConfig, RetryConfig};
pub use auth::Session;

// Re-export streaming module for easy access
pub use streaming::StreamingServices;

/// Prelude module for convenient imports.
///
/// ```rust
/// use tastytrade_rs::prelude::*;
/// ```
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::models::{
        // Primitives
        AccountNumber, OrderId, Symbol, StreamerSymbol, Environment, ApiVersion,
        // Enums
        InstrumentType, OrderType, OrderAction, TimeInForce, OrderStatus,
        PriceEffect, OptionType, TransactionType,
        // Account models
        Account, AccountItem, Customer, AccountBalance, Position,
        // Order models
        NewOrder, NewOrderBuilder, Order, OrderLeg, Fill,
        // Instrument models
        Equity, EquityOption, Future, FutureOption, Cryptocurrency,
        NestedOptionChain,
    };
    pub use crate::client::{TastytradeClient, ClientConfig};
    pub use crate::auth::Session;
    pub use crate::streaming::{
        StreamingServices, DxLinkStreamer, AccountStreamer,
        DxEvent, Quote, Trade, Greeks, Summary, Profile, Candle,
        AccountNotification,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_number_creation() {
        let account = AccountNumber::new("5WV12345");
        assert_eq!(account.as_str(), "5WV12345");
    }

    #[test]
    fn test_environment_urls() {
        assert_eq!(
            Environment::Production.api_base_url(),
            "https://api.tastyworks.com"
        );
        assert_eq!(
            Environment::Sandbox.api_base_url(),
            "https://api.cert.tastyworks.com"
        );
    }

    #[test]
    fn test_api_version_validation() {
        // Valid version
        assert!(ApiVersion::new("20231215").is_ok());

        // Invalid versions
        assert!(ApiVersion::new("2023").is_err());
        assert!(ApiVersion::new("not-a-date").is_err());
    }
}
