# Account Streaming Enhancement Implementation Plan

## Executive Summary

This document provides a detailed implementation plan to address all gaps identified in the [tastytrade-rs gap analysis](/Users/salamad/Downloads/tastytrade-rs-gap-analysis.md). The plan is structured for execution by a Senior Rust Developer and covers five primary gaps plus documentation enhancements.

**Estimated Total Effort**: 3-4 development sessions
**Risk Level**: Low (additive changes, no breaking modifications to existing public API)

---

## Table of Contents

1. [Current Architecture Analysis](#current-architecture-analysis)
2. [Gap 1: Automatic Reconnection (High Priority)](#gap-1-automatic-reconnection)
3. [Gap 2: Subscription State Inspection (Medium Priority)](#gap-2-subscription-state-inspection)
4. [Gap 3: Connection Status Events (Medium Priority)](#gap-3-connection-status-events)
5. [Gap 4: Order ID Matching Helper (Low Priority)](#gap-4-order-id-matching-helper)
6. [Gap 5: Typed Order Status (Low Priority)](#gap-5-typed-order-status)
7. [Documentation Enhancements](#documentation-enhancements)
8. [Testing Strategy](#testing-strategy)
9. [Implementation Order](#implementation-order)

---

## Current Architecture Analysis

### File Structure
```
src/streaming/
├── mod.rs                      # Streaming module exports
├── account/
│   ├── mod.rs                  # AccountStreamer implementation
│   └── notifications.rs        # Notification types (OrderNotification, etc.)
└── dxlink/                     # Market data streaming (separate)
```

### Key Components

| Component | Location | Current State |
|-----------|----------|---------------|
| `AccountStreamer` | `src/streaming/account/mod.rs` | Core struct with WebSocket handling |
| `AccountNotification` | `src/streaming/account/notifications.rs` | Enum with 7 variants |
| `OrderNotification` | `src/streaming/account/notifications.rs` | Struct with `status: Option<String>` |
| `OrderStatus` | `src/models/enums.rs` | Typed enum with `is_terminal()`, `is_working()` |
| `Error::StreamDisconnected` | `src/error.rs` | Exists but not surfaced as notification |

### Current Limitations Identified

1. **No reconnection logic**: WebSocket drops cause permanent disconnection
2. **Status is String-typed**: `OrderNotification.status` is `Option<String>`, not `Option<OrderStatus>`
3. **No connection state visibility**: Cannot query if connected or what's subscribed
4. **Subscriptions tracked but not exposed**: Internal `HashSet<String>` exists but is private
5. **Disconnect not surfaced as event**: Only returned as `Err(StreamDisconnected)`

---

## Gap 1: Automatic Reconnection

**Priority**: High
**Complexity**: Medium
**Impact**: Critical for 24/7 trading bot operation

### 1.1 New Types

**File**: `src/streaming/account/config.rs` (new file)

```rust
//! Account streamer configuration.

use std::time::Duration;

/// Configuration for automatic reconnection behavior.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Whether auto-reconnect is enabled
    pub enabled: bool,
    /// Maximum number of reconnection attempts (0 = unlimited)
    pub max_attempts: u32,
    /// Initial backoff delay
    pub initial_backoff: Duration,
    /// Maximum backoff delay
    pub max_backoff: Duration,
    /// Backoff multiplier (exponential backoff)
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
    /// Create a config with reconnection disabled.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config for aggressive reconnection (trading bots).
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            max_attempts: 0, // Unlimited
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 1.5,
        }
    }

    /// Calculate backoff duration for a given attempt number.
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.initial_backoff.as_millis() as f64;
        let multiplied = base * self.backoff_multiplier.powi(attempt as i32);
        let capped = multiplied.min(self.max_backoff.as_millis() as f64);
        Duration::from_millis(capped as u64)
    }
}
```

### 1.2 Connection State Tracking

**File**: `src/streaming/account/mod.rs` (modifications)

Add new internal state:

```rust
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Internal connection state.
struct ConnectionState {
    /// Whether currently connected
    connected: AtomicBool,
    /// Number of reconnection attempts since last successful connection
    reconnect_attempts: AtomicU32,
    /// Session token for reconnection
    token: RwLock<Option<String>>,
    /// Client reference for reconnection
    client: Arc<ClientInner>,
}
```

### 1.3 Modified AccountStreamer Struct

```rust
pub struct AccountStreamer {
    // Existing fields
    write: Arc<RwLock<SplitSink<...>>>,
    notification_rx: mpsc::Receiver<Result<AccountNotification>>,
    subscriptions: Arc<RwLock<HashSet<String>>>,

    // New fields
    state: Arc<ConnectionState>,
    reconnect_config: ReconnectConfig,
    /// Channel to send internal commands (reconnect trigger)
    command_tx: mpsc::Sender<StreamerCommand>,
}

enum StreamerCommand {
    Reconnect,
    Shutdown,
}
```

### 1.4 New Public Methods

```rust
impl AccountStreamer {
    /// Check if the WebSocket connection is currently active.
    pub fn is_connected(&self) -> bool {
        self.state.connected.load(Ordering::SeqCst)
    }

    /// Manually trigger a reconnection attempt.
    ///
    /// This will:
    /// 1. Close the existing connection (if any)
    /// 2. Establish a new WebSocket connection
    /// 3. Re-authenticate
    /// 4. Re-subscribe to all previously subscribed accounts
    ///
    /// Returns `Ok(())` if reconnection succeeds.
    pub async fn reconnect(&mut self) -> Result<()> {
        self.perform_reconnect().await
    }

    /// Configure automatic reconnection behavior.
    ///
    /// This is a builder-style method for configuring reconnection.
    ///
    /// # Example
    /// ```ignore
    /// let streamer = client.streaming().account().await?
    ///     .with_reconnect_config(ReconnectConfig::aggressive());
    /// ```
    pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }

    /// Get the current reconnection configuration.
    pub fn reconnect_config(&self) -> &ReconnectConfig {
        &self.reconnect_config
    }
}
```

### 1.5 Reconnection Logic Implementation

```rust
impl AccountStreamer {
    async fn perform_reconnect(&mut self) -> Result<()> {
        // Mark as disconnected
        self.state.connected.store(false, Ordering::SeqCst);

        // Close existing connection
        if let Ok(mut write) = self.write.try_write() {
            let _ = write.close().await;
        }

        // Get stored subscriptions before reconnecting
        let subscriptions: Vec<String> = {
            let subs = self.subscriptions.read().await;
            subs.iter().cloned().collect()
        };

        // Attempt reconnection with backoff
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            self.state.reconnect_attempts.store(attempt, Ordering::SeqCst);

            // Check max attempts
            if self.reconnect_config.max_attempts > 0
               && attempt > self.reconnect_config.max_attempts {
                return Err(Error::WebSocket(format!(
                    "Max reconnection attempts ({}) exceeded",
                    self.reconnect_config.max_attempts
                )));
            }

            // Calculate backoff
            let backoff = self.reconnect_config.backoff_for_attempt(attempt - 1);
            tokio::time::sleep(backoff).await;

            // Attempt connection
            match self.try_reconnect_once(&subscriptions).await {
                Ok(()) => {
                    self.state.reconnect_attempts.store(0, Ordering::SeqCst);
                    self.state.connected.store(true, Ordering::SeqCst);
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt,
                        error = %e,
                        "Reconnection attempt failed"
                    );
                    continue;
                }
            }
        }
    }

    async fn try_reconnect_once(&mut self, subscriptions: &[String]) -> Result<()> {
        // Get fresh token
        let token = Self::get_streamer_token(&self.state.client).await?;

        // Store token for future reconnections
        {
            let mut stored_token = self.state.token.write().await;
            *stored_token = Some(token.clone());
        }

        // Connect to WebSocket
        let url = self.state.client.session.environment().await.account_streamer_url();
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        // Update write handle
        *self.write.write().await = write;

        // Re-authenticate
        Self::authenticate(&self.write, &token).await?;

        // Restart message processing
        let (notification_tx, notification_rx) = mpsc::channel(1024);
        self.notification_rx = notification_rx;

        let subs_clone = self.subscriptions.clone();
        let state_clone = self.state.clone();
        let config_clone = self.reconnect_config.clone();
        let command_tx = self.command_tx.clone();

        tokio::spawn(async move {
            Self::process_messages_with_reconnect(
                read,
                notification_tx,
                subs_clone,
                state_clone,
                config_clone,
                command_tx,
            ).await;
        });

        // Re-subscribe to all accounts
        for account in subscriptions {
            let msg = serde_json::json!({
                "action": "account-subscribe",
                "value": [account]
            });
            self.send_message(&msg).await?;
        }

        Ok(())
    }
}
```

### 1.6 Enhanced Message Processing with Auto-Reconnect

```rust
async fn process_messages_with_reconnect(
    mut read: SplitStream<...>,
    notification_tx: mpsc::Sender<Result<AccountNotification>>,
    subscriptions: Arc<RwLock<HashSet<String>>>,
    state: Arc<ConnectionState>,
    config: ReconnectConfig,
    command_tx: mpsc::Sender<StreamerCommand>,
) {
    loop {
        match read.next().await {
            Some(Ok(Message::Text(text))) => {
                if let Ok(notification) = Self::parse_notification(&text) {
                    if notification_tx.send(Ok(notification)).await.is_err() {
                        return;
                    }
                }
            }
            Some(Ok(Message::Ping(data))) => {
                let _ = data; // Pong handled automatically
            }
            Some(Ok(Message::Close(frame))) => {
                let reason = frame
                    .map(|f| f.reason.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                state.connected.store(false, Ordering::SeqCst);

                // Send Disconnected notification
                let _ = notification_tx.send(Ok(
                    AccountNotification::Disconnected { reason: reason.clone() }
                )).await;

                if config.enabled {
                    // Trigger reconnection
                    let _ = command_tx.send(StreamerCommand::Reconnect).await;
                }
                return;
            }
            Some(Err(e)) => {
                state.connected.store(false, Ordering::SeqCst);

                let _ = notification_tx.send(Ok(
                    AccountNotification::Disconnected {
                        reason: e.to_string()
                    }
                )).await;

                if config.enabled {
                    let _ = command_tx.send(StreamerCommand::Reconnect).await;
                }
                return;
            }
            None => {
                // Stream ended
                state.connected.store(false, Ordering::SeqCst);
                let _ = notification_tx.send(Ok(
                    AccountNotification::Disconnected {
                        reason: "Stream ended".to_string()
                    }
                )).await;
                return;
            }
            _ => {}
        }
    }
}
```

---

## Gap 2: Subscription State Inspection

**Priority**: Medium
**Complexity**: Low
**Impact**: Useful for debugging and health checks

### 2.1 New Public Methods

**File**: `src/streaming/account/mod.rs`

```rust
impl AccountStreamer {
    /// Get a list of currently subscribed account numbers.
    ///
    /// This returns a snapshot of the current subscriptions. The actual
    /// server-side subscription state may differ if messages are in flight.
    pub async fn subscribed_accounts(&self) -> Vec<AccountNumber> {
        let subs = self.subscriptions.read().await;
        subs.iter()
            .map(|s| AccountNumber::new(s))
            .collect()
    }

    /// Check if currently subscribed to a specific account.
    pub async fn is_subscribed(&self, account: &AccountNumber) -> bool {
        let subs = self.subscriptions.read().await;
        subs.contains(account.as_str())
    }

    /// Get the count of active subscriptions.
    pub async fn subscription_count(&self) -> usize {
        let subs = self.subscriptions.read().await;
        subs.len()
    }
}
```

### 2.2 Synchronous Variants (for non-async contexts)

```rust
impl AccountStreamer {
    /// Get subscribed accounts synchronously (may block briefly).
    ///
    /// Prefer `subscribed_accounts()` in async contexts.
    pub fn subscribed_accounts_blocking(&self) -> Vec<AccountNumber> {
        // Use try_read to avoid blocking, fall back to empty if locked
        match self.subscriptions.try_read() {
            Ok(subs) => subs.iter().map(|s| AccountNumber::new(s)).collect(),
            Err(_) => Vec::new(),
        }
    }
}
```

---

## Gap 3: Connection Status Events

**Priority**: Medium
**Complexity**: Low
**Impact**: Better observability for trading bots

### 3.1 Extend AccountNotification Enum

**File**: `src/streaming/account/notifications.rs`

```rust
/// Account notification event.
#[derive(Debug, Clone)]
pub enum AccountNotification {
    /// Heartbeat message to keep connection alive
    Heartbeat,

    /// Subscription confirmation from server
    SubscriptionConfirmation {
        /// List of subscribed account numbers
        accounts: Vec<String>,
    },

    /// Order status update
    Order(OrderNotification),

    /// Position update
    Position(PositionNotification),

    /// Balance update
    Balance(BalanceNotification),

    /// Quote alert triggered
    QuoteAlert(QuoteAlertNotification),

    // ===== NEW VARIANTS =====

    /// WebSocket connection was lost.
    ///
    /// After receiving this, no further notifications will arrive until
    /// reconnection succeeds (if auto-reconnect is enabled) or `reconnect()`
    /// is called manually.
    Disconnected {
        /// Reason for disconnection
        reason: String,
    },

    /// Successfully reconnected to the streaming service.
    ///
    /// All previous subscriptions have been automatically restored.
    Reconnected {
        /// Number of accounts re-subscribed
        accounts_restored: usize,
    },

    /// Connection health warning.
    ///
    /// Sent when heartbeat responses are delayed or connection quality degrades.
    ConnectionWarning {
        /// Warning message
        message: String,
    },

    /// Unknown notification type (raw JSON)
    Unknown(serde_json::Value),
}

impl AccountNotification {
    /// Returns `true` if this is a connection lifecycle event.
    pub fn is_connection_event(&self) -> bool {
        matches!(
            self,
            AccountNotification::Disconnected { .. }
                | AccountNotification::Reconnected { .. }
                | AccountNotification::ConnectionWarning { .. }
        )
    }

    /// Returns `true` if this notification indicates the connection is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(
            self,
            AccountNotification::Heartbeat
                | AccountNotification::SubscriptionConfirmation { .. }
                | AccountNotification::Reconnected { .. }
        )
    }
}
```

### 3.2 Send Reconnected Event After Successful Reconnection

In `perform_reconnect()`:

```rust
// After successful reconnection and re-subscription
let accounts_restored = subscriptions.len();
let _ = self.notification_tx.send(Ok(
    AccountNotification::Reconnected { accounts_restored }
)).await;
```

---

## Gap 4: Order ID Matching Helper

**Priority**: Low
**Complexity**: Trivial
**Impact**: Convenience for matching orders

### 4.1 Add Helper Methods to OrderNotification

**File**: `src/streaming/account/notifications.rs`

```rust
impl OrderNotification {
    /// Get the order ID as a string.
    ///
    /// Returns `None` if the order notification doesn't contain an ID
    /// (which should be rare for valid order updates).
    pub fn order_id(&self) -> Option<String> {
        self.id.map(|id| id.to_string())
    }

    /// Get the order ID, panicking if not present.
    ///
    /// # Panics
    /// Panics if `id` is `None`. Use `order_id()` for safe access.
    ///
    /// # Use Case
    /// Use this when you're certain the notification contains an ID
    /// (e.g., in order update handlers).
    pub fn order_id_unchecked(&self) -> String {
        self.id
            .map(|id| id.to_string())
            .expect("OrderNotification missing order ID")
    }

    /// Check if this notification matches a specific order ID.
    pub fn matches_order_id(&self, order_id: &str) -> bool {
        self.order_id().as_deref() == Some(order_id)
    }

    /// Check if this notification matches a specific order ID (i64 variant).
    pub fn matches_order(&self, order_id: i64) -> bool {
        self.id == Some(order_id)
    }
}
```

---

## Gap 5: Typed Order Status

**Priority**: Low
**Complexity**: Low
**Impact**: Type safety, better pattern matching

### 5.1 Modify OrderNotification Status Field

**File**: `src/streaming/account/notifications.rs`

Change from:
```rust
pub status: Option<String>,
```

To:
```rust
use crate::models::OrderStatus;

/// Order status (typed).
///
/// This is deserialized from the string status in the JSON response.
/// If the status string doesn't match a known variant, this will be `None`.
#[serde(default, deserialize_with = "deserialize_order_status")]
pub status: Option<OrderStatus>,

/// Raw status string from the API.
///
/// Preserved for cases where `status` deserialization fails or for debugging.
#[serde(rename = "status")]
pub status_raw: Option<String>,
```

### 5.2 Custom Deserializer for Order Status

```rust
use serde::de::Deserializer;

fn deserialize_order_status<'de, D>(deserializer: D) -> Result<Option<OrderStatus>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;

    match opt {
        Some(s) => {
            // Try to parse the status string into OrderStatus
            match serde_json::from_value::<OrderStatus>(serde_json::Value::String(s.clone())) {
                Ok(status) => Ok(Some(status)),
                Err(_) => {
                    // Log unknown status for debugging
                    tracing::debug!(status = %s, "Unknown order status in notification");
                    Ok(None)
                }
            }
        }
        None => Ok(None),
    }
}
```

### 5.3 Add Convenience Methods

```rust
impl OrderNotification {
    /// Check if this order is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.status.as_ref().map(|s| s.is_terminal()).unwrap_or(false)
    }

    /// Check if this order is working (live, in-flight, etc.).
    pub fn is_working(&self) -> bool {
        self.status.as_ref().map(|s| s.is_working()).unwrap_or(false)
    }

    /// Check if this order was filled.
    pub fn is_filled(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Filled))
    }

    /// Check if this order was rejected.
    pub fn is_rejected(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Rejected))
    }

    /// Check if this order was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.status, Some(OrderStatus::Cancelled))
    }

    /// Get the typed status, falling back to parsing the raw status string.
    pub fn effective_status(&self) -> Option<OrderStatus> {
        self.status.clone().or_else(|| {
            self.status_raw.as_ref().and_then(|s| {
                serde_json::from_value(serde_json::Value::String(s.clone())).ok()
            })
        })
    }
}
```

---

## Documentation Enhancements

### 6.1 Module-Level Documentation

**File**: `src/streaming/account/mod.rs`

Add comprehensive module documentation:

```rust
//! Account streaming for real-time account updates.
//!
//! This module provides real-time account notifications via WebSocket,
//! including order status changes, position updates, and balance changes.
//!
//! # Connection Lifecycle
//!
//! The `AccountStreamer` manages a WebSocket connection to TastyTrade's
//! streaming service. The connection lifecycle is:
//!
//! 1. **Connect**: Call `client.streaming().account().await?`
//! 2. **Subscribe**: Call `streamer.subscribe(&account).await?`
//! 3. **Receive**: Loop on `streamer.next().await`
//! 4. **Handle disconnects**: React to `AccountNotification::Disconnected`
//! 5. **Reconnect**: Automatic (if enabled) or call `streamer.reconnect().await?`
//!
//! # Disconnect Behavior
//!
//! When the WebSocket connection drops:
//!
//! - With auto-reconnect **enabled** (default):
//!   1. `AccountNotification::Disconnected` is emitted
//!   2. Library attempts reconnection with exponential backoff
//!   3. On success, `AccountNotification::Reconnected` is emitted
//!   4. All previous subscriptions are automatically restored
//!
//! - With auto-reconnect **disabled**:
//!   1. `AccountNotification::Disconnected` is emitted
//!   2. `next()` returns `None` on subsequent calls
//!   3. Call `reconnect()` manually to restore connection
//!
//! # Thread Safety
//!
//! `AccountStreamer` is **not** `Send` or `Sync`. It should be used from
//! a single async task. If you need to access subscription state from
//! multiple tasks, clone the relevant data after receiving notifications.
//!
//! # Example: Monitoring Order Fills
//!
//! ```no_run
//! use tastytrade_rs::prelude::*;
//! use tastytrade_rs::streaming::AccountNotification;
//!
//! async fn monitor_fills(
//!     mut streamer: AccountStreamer,
//!     order_id: i64,
//! ) -> Result<(), tastytrade_rs::Error> {
//!     while let Some(event) = streamer.next().await {
//!         match event? {
//!             AccountNotification::Order(order) => {
//!                 if order.matches_order(order_id) {
//!                     if order.is_filled() {
//!                         println!("Order filled at {:?}", order.average_fill_price);
//!                         return Ok(());
//!                     } else if order.is_rejected() {
//!                         println!("Order rejected");
//!                         return Ok(());
//!                     }
//!                 }
//!             }
//!             AccountNotification::Disconnected { reason } => {
//!                 println!("Disconnected: {}", reason);
//!                 // Auto-reconnect will handle this if enabled
//!             }
//!             AccountNotification::Reconnected { accounts_restored } => {
//!                 println!("Reconnected, {} accounts restored", accounts_restored);
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Example: Configuring Reconnection
//!
//! ```no_run
//! use tastytrade_rs::prelude::*;
//! use tastytrade_rs::streaming::ReconnectConfig;
//! use std::time::Duration;
//!
//! # async fn example(client: TastytradeClient) -> tastytrade_rs::Result<()> {
//! // Use aggressive reconnection for trading bots
//! let streamer = client.streaming().account().await?
//!     .with_reconnect_config(ReconnectConfig::aggressive());
//!
//! // Or customize:
//! let custom_config = ReconnectConfig {
//!     enabled: true,
//!     max_attempts: 20,
//!     initial_backoff: Duration::from_millis(100),
//!     max_backoff: Duration::from_secs(30),
//!     backoff_multiplier: 1.5,
//! };
//! let streamer = client.streaming().account().await?
//!     .with_reconnect_config(custom_config);
//! # Ok(())
//! # }
//! ```
```

### 6.2 README Example

Add to `README.md`:

```markdown
### Real-Time Order Monitoring

```rust
use tastytrade_rs::prelude::*;
use tastytrade_rs::streaming::{AccountNotification, ReconnectConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WT00000");

    // Create streamer with aggressive reconnection for 24/7 operation
    let mut streamer = client.streaming().account().await?
        .with_reconnect_config(ReconnectConfig::aggressive());

    // Subscribe to account updates
    streamer.subscribe(&account).await?;

    // Monitor events
    while let Some(event) = streamer.next().await {
        match event? {
            AccountNotification::Order(order) => {
                println!(
                    "Order {}: {:?}",
                    order.order_id().unwrap_or_default(),
                    order.status
                );

                if order.is_filled() {
                    println!("  Filled at: {:?}", order.average_fill_price);
                }
            }
            AccountNotification::Disconnected { reason } => {
                eprintln!("Connection lost: {}", reason);
            }
            AccountNotification::Reconnected { accounts_restored } => {
                println!("Reconnected! {} accounts restored", accounts_restored);
            }
            _ => {}
        }
    }

    Ok(())
}
```
```

---

## Testing Strategy

### 7.1 Unit Tests

**File**: `src/streaming/account/tests.rs` (new file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === ReconnectConfig Tests ===

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 10);
    }

    #[test]
    fn test_reconnect_config_backoff() {
        let config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(60),
            ..Default::default()
        };

        assert_eq!(config.backoff_for_attempt(0), Duration::from_secs(1));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(config.backoff_for_attempt(10), Duration::from_secs(60)); // Capped
    }

    // === OrderNotification Tests ===

    #[test]
    fn test_order_notification_matching() {
        let order = OrderNotification {
            id: Some(12345),
            status: Some(OrderStatus::Filled),
            ..Default::default()
        };

        assert!(order.matches_order(12345));
        assert!(!order.matches_order(99999));
        assert!(order.matches_order_id("12345"));
        assert!(order.is_filled());
        assert!(order.is_terminal());
    }

    #[test]
    fn test_order_notification_status_helpers() {
        let filled = OrderNotification {
            status: Some(OrderStatus::Filled),
            ..Default::default()
        };
        assert!(filled.is_filled());
        assert!(filled.is_terminal());
        assert!(!filled.is_working());

        let live = OrderNotification {
            status: Some(OrderStatus::Live),
            ..Default::default()
        };
        assert!(!live.is_filled());
        assert!(!live.is_terminal());
        assert!(live.is_working());
    }

    // === AccountNotification Tests ===

    #[test]
    fn test_notification_is_connection_event() {
        assert!(AccountNotification::Disconnected {
            reason: "test".into()
        }.is_connection_event());

        assert!(AccountNotification::Reconnected {
            accounts_restored: 1
        }.is_connection_event());

        assert!(!AccountNotification::Heartbeat.is_connection_event());
        assert!(!AccountNotification::Order(Default::default()).is_connection_event());
    }
}
```

### 7.2 Integration Tests

**File**: `tests/streaming_tests.rs`

```rust
//! Integration tests for account streaming.
//!
//! These tests require valid API credentials.

use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_account_streamer_connect_and_subscribe() {
    let client = get_test_client().await;
    let account = get_test_account();

    let mut streamer = client.streaming().account().await
        .expect("Should connect");

    assert!(streamer.is_connected());
    assert_eq!(streamer.subscription_count().await, 0);

    streamer.subscribe(&account).await
        .expect("Should subscribe");

    assert!(streamer.is_subscribed(&account).await);
    assert_eq!(streamer.subscription_count().await, 1);

    // Wait for subscription confirmation
    let result = timeout(Duration::from_secs(5), async {
        while let Some(event) = streamer.next().await {
            if let Ok(AccountNotification::SubscriptionConfirmation { .. }) = event {
                return true;
            }
        }
        false
    }).await;

    assert!(result.unwrap_or(false), "Should receive subscription confirmation");

    streamer.close().await.expect("Should close cleanly");
}

#[tokio::test]
async fn test_reconnect_config() {
    let client = get_test_client().await;

    let streamer = client.streaming().account().await
        .expect("Should connect")
        .with_reconnect_config(ReconnectConfig::aggressive());

    assert!(streamer.reconnect_config().enabled);
    assert_eq!(streamer.reconnect_config().max_attempts, 0); // Unlimited
}
```

### 7.3 Mock-Based Disconnect Tests

```rust
#[tokio::test]
async fn test_disconnect_notification_emitted() {
    // This would use a mock WebSocket server
    // to simulate disconnection scenarios
}
```

---

## Implementation Order

Execute the implementation in this order to minimize risk and maximize incremental value:

### Phase 1: Low-Risk Quick Wins (1 session)

1. **Gap 4: Order ID Helper Methods**
   - Add `order_id()`, `matches_order()` methods
   - Pure additions, no breaking changes
   - Unit tests only

2. **Gap 2: Subscription State Inspection**
   - Add `subscribed_accounts()`, `is_subscribed()` methods
   - Expose existing internal state
   - Unit tests only

3. **Gap 5: Typed Order Status**
   - Add custom deserializer
   - Add `status: Option<OrderStatus>` field
   - Keep `status_raw: Option<String>` for backwards compatibility
   - Add helper methods (`is_filled()`, `is_terminal()`, etc.)

### Phase 2: Connection Events (1 session)

4. **Gap 3: Connection Status Events**
   - Add `Disconnected`, `Reconnected`, `ConnectionWarning` variants
   - Modify `process_messages()` to emit `Disconnected`
   - Update parse logic

### Phase 3: Auto-Reconnection (1-2 sessions)

5. **Gap 1: Automatic Reconnection**
   - Create `ReconnectConfig` struct
   - Add `ConnectionState` tracking
   - Implement `is_connected()`, `reconnect()` methods
   - Implement auto-reconnect loop
   - Comprehensive integration testing

### Phase 4: Documentation & Polish (1 session)

6. **Documentation**
   - Add module-level docs
   - Add README examples
   - Add docstrings to all new methods

7. **Final Testing**
   - End-to-end testing with sandbox account
   - Edge case testing (rapid disconnect/reconnect)
   - Performance validation

---

## Dependency Updates

No new dependencies required. Existing dependencies are sufficient:

| Dependency | Usage | Current |
|------------|-------|---------|
| `tokio` | Async runtime, channels, timers | ✅ |
| `tokio-tungstenite` | WebSocket client | ✅ |
| `futures-util` | Stream utilities | ✅ |
| `serde` / `serde_json` | Serialization | ✅ |
| `tracing` | Logging (optional) | ✅ |

---

## Checklist

### Gap 1: Auto-Reconnection
- [ ] Create `src/streaming/account/config.rs`
- [ ] Add `ReconnectConfig` struct
- [ ] Add `ConnectionState` internal struct
- [ ] Add `is_connected()` method
- [ ] Add `reconnect()` method
- [ ] Add `with_reconnect_config()` builder
- [ ] Implement reconnection loop in message processor
- [ ] Unit tests for `ReconnectConfig`
- [ ] Integration tests for reconnection

### Gap 2: Subscription Inspection
- [ ] Add `subscribed_accounts()` method
- [ ] Add `is_subscribed()` method
- [ ] Add `subscription_count()` method
- [ ] Unit tests

### Gap 3: Connection Events
- [ ] Add `Disconnected` variant
- [ ] Add `Reconnected` variant
- [ ] Add `ConnectionWarning` variant
- [ ] Add `is_connection_event()` helper
- [ ] Update message processor to emit events
- [ ] Unit tests

### Gap 4: Order ID Helper
- [ ] Add `order_id()` method
- [ ] Add `order_id_unchecked()` method
- [ ] Add `matches_order()` method
- [ ] Add `matches_order_id()` method
- [ ] Unit tests

### Gap 5: Typed Order Status
- [ ] Add custom deserializer
- [ ] Change `status` field type
- [ ] Add `status_raw` field
- [ ] Add `is_filled()`, `is_rejected()`, `is_cancelled()` methods
- [ ] Add `is_terminal()`, `is_working()` methods
- [ ] Add `effective_status()` method
- [ ] Unit tests

### Documentation
- [ ] Module-level documentation
- [ ] README examples
- [ ] Docstrings on all new public items
- [ ] CHANGELOG entry

---

## Conclusion

This implementation plan addresses all five gaps identified in the analysis while maintaining backward compatibility with the existing API. The phased approach allows for incremental delivery and testing, with the highest-impact features (auto-reconnection) tackled after the foundational work is complete.

The resulting library will be production-ready for 24/7 automated trading systems, with robust connection handling, clear observability, and type-safe order processing.
