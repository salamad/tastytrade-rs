//! Account streamer configuration.
//!
//! This module provides configuration options for the account streamer,
//! including reconnection behavior settings.

use std::time::Duration;

/// Configuration for automatic reconnection behavior.
///
/// This controls how the `AccountStreamer` handles connection drops.
/// By default, auto-reconnection is enabled with sensible defaults.
///
/// # Example
///
/// ```
/// use tastytrade_rs::streaming::ReconnectConfig;
/// use std::time::Duration;
///
/// // Use default configuration
/// let config = ReconnectConfig::default();
/// assert!(config.enabled);
///
/// // Disable auto-reconnect
/// let config = ReconnectConfig::disabled();
/// assert!(!config.enabled);
///
/// // Aggressive reconnection for trading bots
/// let config = ReconnectConfig::aggressive();
/// assert_eq!(config.max_attempts, 0); // Unlimited
/// ```
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Whether auto-reconnect is enabled.
    ///
    /// If `false`, the streamer will not attempt to reconnect automatically
    /// when the connection drops. You can still call `reconnect()` manually.
    pub enabled: bool,

    /// Maximum number of reconnection attempts.
    ///
    /// Set to `0` for unlimited attempts. Default is `10`.
    pub max_attempts: u32,

    /// Initial backoff delay between reconnection attempts.
    ///
    /// The delay increases exponentially with each attempt up to `max_backoff`.
    pub initial_backoff: Duration,

    /// Maximum backoff delay.
    ///
    /// The backoff delay will never exceed this value.
    pub max_backoff: Duration,

    /// Backoff multiplier for exponential backoff.
    ///
    /// After each failed attempt, the delay is multiplied by this value.
    /// Default is `2.0` (doubles each time).
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}

impl ReconnectConfig {
    /// Create a new configuration with the given settings.
    pub fn new(
        enabled: bool,
        max_attempts: u32,
        initial_backoff: Duration,
        max_backoff: Duration,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            enabled,
            max_attempts,
            initial_backoff,
            max_backoff,
            backoff_multiplier,
        }
    }

    /// Create a configuration with reconnection disabled.
    ///
    /// Use this when you want to handle reconnection manually.
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::ReconnectConfig;
    ///
    /// let config = ReconnectConfig::disabled();
    /// assert!(!config.enabled);
    /// ```
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a configuration for aggressive reconnection.
    ///
    /// This is suitable for trading bots that need to maintain
    /// connection at all costs:
    /// - Unlimited reconnection attempts
    /// - Faster initial backoff (500ms)
    /// - Lower maximum backoff (30s)
    /// - Slower backoff growth (1.5x)
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::ReconnectConfig;
    ///
    /// let config = ReconnectConfig::aggressive();
    /// assert!(config.enabled);
    /// assert_eq!(config.max_attempts, 0); // Unlimited
    /// ```
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            max_attempts: 0, // Unlimited
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 1.5,
        }
    }

    /// Create a configuration for conservative reconnection.
    ///
    /// This limits reconnection attempts to avoid overwhelming
    /// the server during outages:
    /// - Maximum 5 attempts
    /// - Longer backoffs (2s to 120s)
    /// - Standard exponential growth
    pub fn conservative() -> Self {
        Self {
            enabled: true,
            max_attempts: 5,
            initial_backoff: Duration::from_secs(2),
            max_backoff: Duration::from_secs(120),
            backoff_multiplier: 2.0,
        }
    }

    /// Calculate the backoff duration for a given attempt number.
    ///
    /// The attempt number is 0-indexed (first attempt = 0).
    ///
    /// # Example
    /// ```
    /// use tastytrade_rs::streaming::ReconnectConfig;
    /// use std::time::Duration;
    ///
    /// let config = ReconnectConfig::default();
    /// assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(1));
    /// assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(2));
    /// assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(4));
    /// ```
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let base_ms = self.initial_backoff.as_millis() as f64;
        let multiplied = base_ms * self.backoff_multiplier.powi(attempt as i32);
        let capped = multiplied.min(self.max_backoff.as_millis() as f64);
        Duration::from_millis(capped as u64)
    }

    /// Check if the maximum number of attempts has been exceeded.
    ///
    /// Returns `false` if `max_attempts` is 0 (unlimited).
    pub fn max_attempts_exceeded(&self, attempts: u32) -> bool {
        self.max_attempts > 0 && attempts >= self.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReconnectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.initial_backoff, Duration::from_secs(1));
        assert_eq!(config.max_backoff, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_disabled_config() {
        let config = ReconnectConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_aggressive_config() {
        let config = ReconnectConfig::aggressive();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 0); // Unlimited
        assert_eq!(config.initial_backoff, Duration::from_millis(500));
        assert_eq!(config.max_backoff, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_conservative_config() {
        let config = ReconnectConfig::conservative();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 5);
    }

    #[test]
    fn test_backoff_exponential_growth() {
        let config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            ..Default::default()
        };

        assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(1));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(config.backoff_for_attempt(3), Duration::from_secs(8));
        assert_eq!(config.backoff_for_attempt(4), Duration::from_secs(16));
        assert_eq!(config.backoff_for_attempt(5), Duration::from_secs(32));
    }

    #[test]
    fn test_backoff_capped_at_max() {
        let config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(10),
            ..Default::default()
        };

        assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(1));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(config.backoff_for_attempt(3), Duration::from_secs(8));
        assert_eq!(config.backoff_for_attempt(4), Duration::from_secs(10)); // Capped
        assert_eq!(config.backoff_for_attempt(5), Duration::from_secs(10)); // Still capped
        assert_eq!(config.backoff_for_attempt(10), Duration::from_secs(10)); // Still capped
    }

    #[test]
    fn test_max_attempts_exceeded_with_limit() {
        let config = ReconnectConfig {
            max_attempts: 5,
            ..Default::default()
        };

        assert!(!config.max_attempts_exceeded(0));
        assert!(!config.max_attempts_exceeded(4));
        assert!(config.max_attempts_exceeded(5));
        assert!(config.max_attempts_exceeded(10));
    }

    #[test]
    fn test_max_attempts_exceeded_unlimited() {
        let config = ReconnectConfig {
            max_attempts: 0, // Unlimited
            ..Default::default()
        };

        assert!(!config.max_attempts_exceeded(0));
        assert!(!config.max_attempts_exceeded(100));
        assert!(!config.max_attempts_exceeded(1000));
    }

    #[test]
    fn test_aggressive_backoff() {
        let config = ReconnectConfig::aggressive();

        // First few attempts
        assert_eq!(config.backoff_for_attempt(0), Duration::from_millis(500));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_millis(750));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_millis(1125));
    }
}
