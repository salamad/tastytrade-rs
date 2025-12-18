//! Data models for the TastyTrade API.
//!
//! This module contains all the strongly-typed data structures used to
//! interact with the TastyTrade API. Models are organized by domain:
//!
//! - [`primitives`] - Core types like `AccountNumber`, `Symbol`, etc.
//! - [`enums`] - Enumeration types for order types, statuses, etc.
//! - [`account`] - Account and customer models
//! - [`balance`] - Balance and position models
//! - [`order`] - Order-related models
//! - [`instrument`] - Financial instrument models
//! - [`market_data`] - Market data snapshot models
//! - [`trading`] - Trading-specific models (fees, buying power, etc.)

pub mod primitives;
pub mod enums;
pub mod account;
pub mod balance;
pub mod order;
pub mod instrument;
pub mod market_data;
pub mod trading;

// Re-export commonly used types
pub use primitives::*;
pub use enums::*;
pub use account::*;
pub use balance::*;
pub use order::*;
pub use instrument::*;
pub use market_data::*;
pub use trading::*;
