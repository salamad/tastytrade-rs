//! Client configuration options.

use std::time::Duration;

use crate::ApiVersion;

/// Configuration for the TastyTrade client.
///
/// # Example
///
/// ```
/// use tastytrade_rs::ClientConfig;
/// use std::time::Duration;
///
/// let config = ClientConfig::default()
///     .with_timeout(Duration::from_secs(60))
///     .with_user_agent("my-app/1.0");
/// ```
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Request timeout
    pub timeout: Duration,
    /// User-Agent header value
    pub user_agent: String,
    /// Retry configuration
    pub retry: RetryConfig,
    /// Optional API version to pin to
    pub api_version: Option<ApiVersion>,
    /// Whether to automatically refresh expired sessions
    pub auto_refresh_session: bool,
    /// Buffer time (in seconds) before expiry to refresh
    pub refresh_buffer_secs: i64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: format!(
                "tastytrade-rs/{} (Rust)",
                env!("CARGO_PKG_VERSION")
            ),
            retry: RetryConfig::default(),
            api_version: None,
            auto_refresh_session: true,
            refresh_buffer_secs: 60,
        }
    }
}

impl ClientConfig {
    /// Create a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the User-Agent header.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Set the retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Pin to a specific API version.
    pub fn with_api_version(mut self, version: ApiVersion) -> Self {
        self.api_version = Some(version);
        self
    }

    /// Enable or disable automatic session refresh.
    pub fn with_auto_refresh(mut self, enabled: bool) -> Self {
        self.auto_refresh_session = enabled;
        self
    }

    /// Set the buffer time before expiry to refresh.
    pub fn with_refresh_buffer(mut self, secs: i64) -> Self {
        self.refresh_buffer_secs = secs;
        self
    }
}

/// Configuration for automatic retries.
///
/// By default, the client will retry idempotent requests (GET, HEAD)
/// on transient errors with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// HTTP status codes to retry on
    pub retry_statuses: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            retry_statuses: vec![429, 500, 502, 503, 504],
        }
    }
}

impl RetryConfig {
    /// Create a configuration with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Set the initial backoff duration.
    pub fn with_initial_backoff(mut self, duration: Duration) -> Self {
        self.initial_backoff = duration;
        self
    }

    /// Set the maximum backoff duration.
    pub fn with_max_backoff(mut self, duration: Duration) -> Self {
        self.max_backoff = duration;
        self
    }

    /// Calculate the backoff duration for a given attempt.
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_millis = self.initial_backoff.as_millis() as u64 * 2u64.pow(attempt);
        let max_millis = self.max_backoff.as_millis() as u64;
        Duration::from_millis(backoff_millis.min(max_millis))
    }

    /// Check if a status code should be retried.
    pub fn should_retry_status(&self, status: u16) -> bool {
        self.retry_statuses.contains(&status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.auto_refresh_session);
    }

    #[test]
    fn test_retry_backoff() {
        let config = RetryConfig::default();
        assert_eq!(config.backoff_for_attempt(0), Duration::from_millis(500));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_millis(1000));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_millis(2000));
    }

    #[test]
    fn test_retry_backoff_max() {
        let config = RetryConfig::default()
            .with_initial_backoff(Duration::from_secs(10))
            .with_max_backoff(Duration::from_secs(30));

        // 10 * 2^3 = 80, but capped at 30
        assert_eq!(config.backoff_for_attempt(3), Duration::from_secs(30));
    }

    #[test]
    fn test_should_retry_status() {
        let config = RetryConfig::default();
        assert!(config.should_retry_status(429));
        assert!(config.should_retry_status(503));
        assert!(!config.should_retry_status(404));
        assert!(!config.should_retry_status(401));
    }
}
