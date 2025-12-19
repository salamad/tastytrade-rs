//! HTTP client and service layer for the TastyTrade API.
//!
//! This module provides the main entry point [`TastytradeClient`] for
//! interacting with the TastyTrade API.
//!
//! # Example
//!
//! ```no_run
//! use tastytrade_rs::{TastytradeClient, Environment};
//!
//! # async fn example() -> tastytrade_rs::Result<()> {
//! let client = TastytradeClient::from_oauth(
//!     "client-secret",
//!     "refresh-token",
//!     Environment::Sandbox,
//! ).await?;
//!
//! // Get accounts
//! let accounts = client.accounts().list().await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod http;
pub mod paginated;

pub use config::{ClientConfig, RetryConfig};
pub use http::TastytradeClient;
pub use paginated::{PaginatedStream, PaginationInfo, DEFAULT_PAGE_SIZE};
pub(crate) use http::ClientInner;
