//! Error types for the TastyTrade API client.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes when interacting with the TastyTrade API.

use serde_json::Value;
use thiserror::Error;

/// A specialized `Result` type for TastyTrade operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The main error type for all TastyTrade API operations.
///
/// This enum covers all possible error conditions that can occur when
/// using this crate, from network errors to authentication failures
/// to order rejections.
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request failed
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// API returned an error response
    #[error("API error: status={status}, code={code:?}, message={message}")]
    Api {
        /// HTTP status code
        status: u16,
        /// Optional error code from the API
        code: Option<String>,
        /// Human-readable error message
        message: String,
        /// Raw response body for debugging
        body: Value,
    },

    /// Authentication failed (invalid credentials, token exchange failure)
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Session token has expired and needs refresh
    #[error("Session expired; refresh required")]
    SessionExpired,

    /// Order was rejected by the exchange or broker
    #[error("Order rejected: {reason}")]
    OrderRejected {
        /// Reason for rejection
        reason: String,
    },

    /// WebSocket connection error
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Invalid input provided to a function
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Rate limited by the API
    #[error("Rate limited; retry after {retry_after_secs} seconds")]
    RateLimited {
        /// Number of seconds to wait before retrying
        retry_after_secs: u64,
    },

    /// Request timed out
    #[error("Request timeout")]
    Timeout,

    /// Stream was disconnected unexpectedly
    #[error("Stream disconnected")]
    StreamDisconnected,

    /// Invalid symbol provided
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    /// URL parsing error
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Channel send error (internal)
    #[error("Internal channel error")]
    ChannelError,

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Resource not found (404)
    #[error("Not found: {0}")]
    NotFound(String),
}

impl Error {
    /// Returns `true` if this error is potentially transient and the
    /// operation could be retried.
    ///
    /// # Example
    ///
    /// ```
    /// use tastytrade_rs::Error;
    ///
    /// fn handle_error(err: Error) {
    ///     if err.is_retryable() {
    ///         println!("Retrying operation...");
    ///     }
    /// }
    /// ```
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Http(_) | Error::Timeout | Error::RateLimited { .. } | Error::WebSocket(_)
        )
    }

    /// Returns `true` if this is an authentication-related error.
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Error::Authentication(_) | Error::SessionExpired)
    }

    /// Returns `true` if this error indicates a client-side issue
    /// (invalid input, bad request, etc.).
    pub fn is_client_error(&self) -> bool {
        match self {
            Error::Api { status, .. } => *status >= 400 && *status < 500,
            Error::InvalidInput(_) | Error::InvalidSymbol(_) | Error::Config(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if this error indicates a server-side issue.
    pub fn is_server_error(&self) -> bool {
        match self {
            Error::Api { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Create an API error from a response
    pub(crate) fn from_api_response(status: u16, body: Value) -> Self {
        let code = body
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .map(String::from);

        let message = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown API error")
            .to_string();

        Error::Api {
            status,
            code,
            message,
            body,
        }
    }
}

#[cfg(feature = "streaming")]
impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(Error::Timeout.is_retryable());
        assert!(Error::RateLimited { retry_after_secs: 30 }.is_retryable());
        assert!(!Error::InvalidInput("bad".into()).is_retryable());
    }

    #[test]
    fn test_error_auth() {
        assert!(Error::SessionExpired.is_auth_error());
        assert!(Error::Authentication("failed".into()).is_auth_error());
        assert!(!Error::Timeout.is_auth_error());
    }

    #[test]
    fn test_from_api_response() {
        let body = serde_json::json!({
            "error": {
                "code": "INVALID_ORDER",
                "message": "Order validation failed"
            }
        });

        let err = Error::from_api_response(400, body);
        match err {
            Error::Api {
                status,
                code,
                message,
                ..
            } => {
                assert_eq!(status, 400);
                assert_eq!(code, Some("INVALID_ORDER".to_string()));
                assert_eq!(message, "Order validation failed");
            }
            _ => panic!("Expected Api error"),
        }
    }
}
