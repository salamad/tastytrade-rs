//! Primitive types and newtypes for type-safe API interactions.
//!
//! This module provides strongly-typed wrappers around string identifiers
//! to prevent mixing up different types of IDs at compile time.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A strongly-typed account number.
///
/// # Example
///
/// ```
/// use tastytrade_rs::AccountNumber;
///
/// let account = AccountNumber::new("5WV12345");
/// println!("Account: {}", account);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountNumber(String);

impl AccountNumber {
    /// Create a new account number from a string.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the account number as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AccountNumber {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for AccountNumber {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AccountNumber {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A strongly-typed order ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderId(String);

impl OrderId {
    /// Create a new order ID.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the order ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for OrderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for OrderId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for OrderId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A trading symbol (e.g., "AAPL", "SPY").
///
/// # Example
///
/// ```
/// use tastytrade_rs::Symbol;
///
/// let symbol = Symbol::new("AAPL");
/// assert_eq!(symbol.as_str(), "AAPL");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    /// Create a new symbol.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the symbol as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A DXLink-compatible streamer symbol.
///
/// These symbols may differ from standard trading symbols and are used
/// specifically for market data streaming via DXLink.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StreamerSymbol(String);

impl StreamerSymbol {
    /// Create a new streamer symbol.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the streamer symbol as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamerSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for StreamerSymbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for StreamerSymbol {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for StreamerSymbol {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<Symbol> for StreamerSymbol {
    fn from(s: Symbol) -> Self {
        Self(s.0)
    }
}

/// API version in YYYYMMDD format.
///
/// TastyTrade supports date-based API versioning. This type ensures
/// that only valid version strings are used.
///
/// # Example
///
/// ```
/// use tastytrade_rs::ApiVersion;
///
/// let version = ApiVersion::new("20241201").expect("valid version");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiVersion(String);

impl ApiVersion {
    /// Create a new API version, validating the format.
    ///
    /// # Errors
    ///
    /// Returns an error if the version is not in YYYYMMDD format.
    pub fn new(version: &str) -> crate::Result<Self> {
        if version.len() != 8 {
            return Err(crate::Error::InvalidInput(format!(
                "Invalid API version format: {}. Expected YYYYMMDD",
                version
            )));
        }

        // Validate it's a valid date
        if version.parse::<u32>().is_err() {
            return Err(crate::Error::InvalidInput(format!(
                "Invalid API version format: {}. Expected YYYYMMDD",
                version
            )));
        }

        // Basic validation of date components
        let year: u32 = version[0..4].parse().unwrap();
        let month: u32 = version[4..6].parse().unwrap();
        let day: u32 = version[6..8].parse().unwrap();

        if !(2020..=2100).contains(&year) {
            return Err(crate::Error::InvalidInput(format!(
                "Invalid year in API version: {}",
                year
            )));
        }
        if !(1..=12).contains(&month) {
            return Err(crate::Error::InvalidInput(format!(
                "Invalid month in API version: {}",
                month
            )));
        }
        if !(1..=31).contains(&day) {
            return Err(crate::Error::InvalidInput(format!(
                "Invalid day in API version: {}",
                day
            )));
        }

        Ok(ApiVersion(version.to_string()))
    }

    /// Get the version as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Environment configuration for the TastyTrade API.
///
/// Determines which API endpoints to use - production or sandbox.
///
/// # Example
///
/// ```
/// use tastytrade_rs::Environment;
///
/// let env = Environment::Sandbox;
/// println!("API URL: {}", env.api_base_url());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    /// Production environment - real trading with real money.
    #[default]
    Production,
    /// Sandbox/certification environment for testing.
    /// Market quotes are 15 minutes delayed.
    Sandbox,
}

impl Environment {
    /// Get the base URL for REST API requests.
    pub fn api_base_url(&self) -> &'static str {
        match self {
            Environment::Production => "https://api.tastyworks.com",
            Environment::Sandbox => "https://api.cert.tastyworks.com",
        }
    }

    /// Get the WebSocket URL for the account streamer.
    pub fn account_streamer_url(&self) -> &'static str {
        match self {
            Environment::Production => "wss://streamer.tastyworks.com",
            Environment::Sandbox => "wss://streamer.cert.tastyworks.com",
        }
    }

    /// Get the WebSocket URL for DXLink market data streaming.
    pub fn dxlink_url(&self) -> &'static str {
        "wss://tasty-openapi-ws.dxfeed.com/realtime"
    }

    /// Returns `true` if this is the production environment.
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }

    /// Returns `true` if this is the sandbox environment.
    pub fn is_sandbox(&self) -> bool {
        matches!(self, Environment::Sandbox)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Environment::Production => write!(f, "production"),
            Environment::Sandbox => write!(f, "sandbox"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_number() {
        let account = AccountNumber::new("5WV12345");
        assert_eq!(account.as_str(), "5WV12345");
        assert_eq!(account.to_string(), "5WV12345");
    }

    #[test]
    fn test_symbol() {
        let symbol: Symbol = "AAPL".into();
        assert_eq!(symbol.as_str(), "AAPL");
    }

    #[test]
    fn test_api_version_valid() {
        let version = ApiVersion::new("20241201").unwrap();
        assert_eq!(version.as_str(), "20241201");
    }

    #[test]
    fn test_api_version_invalid() {
        assert!(ApiVersion::new("2024").is_err());
        assert!(ApiVersion::new("20241301").is_err()); // Invalid month
        assert!(ApiVersion::new("abcdefgh").is_err());
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
}
