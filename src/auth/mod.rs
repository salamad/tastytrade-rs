//! Authentication and session management for the TastyTrade API.
//!
//! This module provides two authentication methods:
//!
//! 1. **OAuth** (recommended) - Uses client credentials and refresh tokens
//! 2. **Legacy Session** - Uses username/password (for compatibility)
//!
//! # OAuth Authentication
//!
//! OAuth is the recommended authentication method. It uses a long-lived
//! refresh token to obtain short-lived access tokens (~15 minutes).
//!
//! ```no_run
//! use tastytrade_rs::{Session, Environment};
//!
//! # async fn example() -> tastytrade_rs::Result<()> {
//! let session = Session::from_oauth(
//!     "your-client-secret",
//!     "your-refresh-token",
//!     Environment::Sandbox,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Legacy Session Authentication
//!
//! For compatibility, username/password login is also supported:
//!
//! ```no_run
//! use tastytrade_rs::{Session, Environment};
//!
//! # async fn example() -> tastytrade_rs::Result<()> {
//! let session = Session::from_credentials(
//!     "username",
//!     "password",
//!     true, // remember_me
//!     Environment::Sandbox,
//! ).await?;
//! # Ok(())
//! # }
//! ```

mod session;

pub use session::Session;
