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
//! # Example
//!
//! ```ignore
//! use tastytrade_rs::streaming::{AccountStreamer, ReconnectConfig};
//!
//! let mut streamer = client.streaming().account().await?
//!     .with_reconnect_config(ReconnectConfig::aggressive());
//!
//! streamer.subscribe(&account).await?;
//!
//! while let Some(event) = streamer.next().await {
//!     match event? {
//!         AccountNotification::Order(order) if order.is_filled() => {
//!             println!("Order {} filled!", order.order_id().unwrap_or_default());
//!         }
//!         AccountNotification::Disconnected { reason } => {
//!             println!("Disconnected: {}", reason);
//!         }
//!         _ => {}
//!     }
//! }
//! ```

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ClientInner;
use crate::models::AccountNumber;
use crate::{Error, Result};

/// Heartbeat interval in seconds (per TastyTrade protocol)
const HEARTBEAT_INTERVAL_SECS: u64 = 15;

mod config;
mod notifications;

pub use config::*;
pub use notifications::*;

/// Account activity streamer.
///
/// Provides real-time notifications for account activity including:
/// - Order status changes
/// - Position updates
/// - Balance changes
/// - Account alerts
///
/// # Connection Management
///
/// The streamer automatically tracks connection state and can be configured
/// to automatically reconnect when the connection drops. Use `is_connected()`
/// to check the current connection state and `reconnect()` to manually
/// trigger a reconnection attempt.
///
/// # Example
///
/// ```ignore
/// let mut streamer = client.streaming().account().await?;
///
/// // Check connection state
/// if streamer.is_connected() {
///     println!("Connected!");
/// }
///
/// // Configure reconnection
/// let streamer = streamer.with_reconnect_config(ReconnectConfig::aggressive());
/// ```
pub struct AccountStreamer {
    write: Arc<RwLock<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >>>,
    notification_rx: mpsc::Receiver<Result<AccountNotification>>,
    subscriptions: Arc<RwLock<HashSet<String>>>,

    // Connection state tracking
    connected: Arc<AtomicBool>,
    client: Arc<ClientInner>,
    reconnect_config: ReconnectConfig,

    // Authentication token (Bearer + session token)
    auth_token: Arc<RwLock<String>>,
    // Request ID counter for message tracking
    request_id: Arc<AtomicU64>,

    // Auto-reconnection state
    reconnect_attempts: u32,
    // Flag to track if we're in the middle of an auto-reconnect attempt
    auto_reconnecting: bool,
}

/// Account notification action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationAction {
    /// Heartbeat message
    Heartbeat,
    /// Subscription confirmation
    SubscriptionConfirmation,
    /// Order data notification
    OrderData,
    /// Position data notification
    PositionData,
    /// Balance data notification
    BalanceData,
    /// Account alert
    Alert,
    /// Public watchlist data
    PublicWatchlistsData,
    /// Quote alert
    QuoteAlert,
}

impl AccountStreamer {
    /// Connect to the account streaming service.
    pub(crate) async fn connect(client: Arc<ClientInner>) -> Result<Self> {
        // Get the session token (used directly without Bearer prefix for session login)
        let session_token = client.session.session_token().await;
        let auth_token = Arc::new(RwLock::new(session_token));

        // Connect to WebSocket
        let url = client.session.environment().await.account_streamer_url();
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        let write = Arc::new(RwLock::new(write));
        let subscriptions = Arc::new(RwLock::new(HashSet::new()));
        let connected = Arc::new(AtomicBool::new(true));
        let request_id = Arc::new(AtomicU64::new(1));

        // Create notification channel
        let (notification_tx, notification_rx) = mpsc::channel(1024);

        // Start message processing task
        let subs_clone = subscriptions.clone();
        let connected_clone = connected.clone();
        tokio::spawn(async move {
            Self::process_messages(read, notification_tx, subs_clone, connected_clone).await;
        });

        // Start heartbeat task
        let heartbeat_write = write.clone();
        let heartbeat_token = auth_token.clone();
        let heartbeat_connected = connected.clone();
        let heartbeat_request_id = request_id.clone();
        tokio::spawn(async move {
            Self::heartbeat_task(
                heartbeat_write,
                heartbeat_token,
                heartbeat_connected,
                heartbeat_request_id,
            )
            .await;
        });

        Ok(Self {
            write,
            notification_rx,
            subscriptions,
            connected,
            client,
            reconnect_config: ReconnectConfig::default(),
            auth_token,
            request_id,
            reconnect_attempts: 0,
            auto_reconnecting: false,
        })
    }

    // ===== Connection State Methods (Gap 1) =====

    /// Check if the WebSocket connection is currently active.
    ///
    /// This returns `true` if the connection is established and healthy.
    /// Note that this is an atomic check and the state may change immediately after.
    ///
    /// # Example
    /// ```ignore
    /// if streamer.is_connected() {
    ///     println!("Connection is active");
    /// } else {
    ///     println!("Connection lost, attempting reconnect...");
    ///     streamer.reconnect().await?;
    /// }
    /// ```
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Get the current reconnection configuration.
    pub fn reconnect_config(&self) -> &ReconnectConfig {
        &self.reconnect_config
    }

    /// Configure automatic reconnection behavior.
    ///
    /// This is a builder-style method for configuring reconnection.
    /// Call this immediately after connecting.
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

    /// Manually trigger a reconnection attempt.
    ///
    /// This will:
    /// 1. Close the existing connection (if any)
    /// 2. Establish a new WebSocket connection
    /// 3. Re-authenticate
    /// 4. Re-subscribe to all previously subscribed accounts
    ///
    /// Returns `Ok(())` if reconnection succeeds.
    ///
    /// # Example
    /// ```ignore
    /// match streamer.reconnect().await {
    ///     Ok(()) => println!("Reconnected successfully"),
    ///     Err(e) => println!("Reconnection failed: {}", e),
    /// }
    /// ```
    pub async fn reconnect(&mut self) -> Result<()> {
        // Mark as disconnected
        self.connected.store(false, Ordering::SeqCst);

        // Get current subscriptions to restore after reconnect
        let subscriptions: Vec<String> = {
            let subs = self.subscriptions.read().await;
            subs.iter().cloned().collect()
        };

        // Ensure session is valid (refresh if expired)
        self.client.session.ensure_valid().await?;

        // Get fresh session token (used directly without Bearer prefix for session login)
        let session_token = self.client.session.session_token().await;
        {
            let mut token = self.auth_token.write().await;
            *token = session_token;
        }

        // Connect to WebSocket
        let url = self.client.session.environment().await.account_streamer_url();
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        // Update write handle
        *self.write.write().await = write;

        // Create new notification channel
        let (notification_tx, notification_rx) = mpsc::channel(1024);
        self.notification_rx = notification_rx;

        // Mark as connected
        self.connected.store(true, Ordering::SeqCst);

        // Start message processing task
        let subs_clone = self.subscriptions.clone();
        let connected_clone = self.connected.clone();
        tokio::spawn(async move {
            Self::process_messages(read, notification_tx, subs_clone, connected_clone).await;
        });

        // Start new heartbeat task
        let heartbeat_write = self.write.clone();
        let heartbeat_token = self.auth_token.clone();
        let heartbeat_connected = self.connected.clone();
        let heartbeat_request_id = self.request_id.clone();
        tokio::spawn(async move {
            Self::heartbeat_task(
                heartbeat_write,
                heartbeat_token,
                heartbeat_connected,
                heartbeat_request_id,
            )
            .await;
        });

        // Re-subscribe to all accounts using the "connect" action
        if !subscriptions.is_empty() {
            self.send_authenticated_message("connect", Some(subscriptions))
                .await?;
        }

        Ok(())
    }

    /// Subscribe to account updates.
    ///
    /// This will receive notifications for order changes, position updates,
    /// and balance changes for the specified account.
    ///
    /// The subscription uses the "connect" action per TastyTrade's protocol.
    pub async fn subscribe(&mut self, account_number: &AccountNumber) -> Result<()> {
        // Add to local subscription set first
        {
            let mut subs = self.subscriptions.write().await;
            subs.insert(account_number.to_string());
        }

        // Get all current subscriptions for the connect message
        let all_accounts: Vec<String> = {
            let subs = self.subscriptions.read().await;
            subs.iter().cloned().collect()
        };

        // Send connect message with all subscribed accounts
        self.send_authenticated_message("connect", Some(all_accounts))
            .await
    }

    /// Subscribe to quote alerts.
    ///
    /// Receive notifications when price alerts are triggered.
    pub async fn subscribe_quote_alerts(&mut self) -> Result<()> {
        self.send_authenticated_message("quote-alerts-subscribe", None::<Vec<String>>)
            .await
    }

    /// Subscribe to public watchlists.
    ///
    /// Receive notifications for public watchlist updates.
    pub async fn subscribe_public_watchlists(&mut self) -> Result<()> {
        self.send_authenticated_message("public-watchlists-subscribe", None::<Vec<String>>)
            .await
    }

    /// Unsubscribe from account updates.
    ///
    /// Note: This removes the account from local tracking. To fully disconnect,
    /// you would need to send a new "connect" message with the updated account list.
    pub async fn unsubscribe(&mut self, account_number: &AccountNumber) -> Result<()> {
        // Remove from local subscription set
        {
            let mut subs = self.subscriptions.write().await;
            subs.remove(account_number.as_str());
        }

        // Get remaining subscriptions
        let remaining_accounts: Vec<String> = {
            let subs = self.subscriptions.read().await;
            subs.iter().cloned().collect()
        };

        // If there are remaining accounts, re-send connect with updated list
        // If no accounts remain, we don't need to send anything (connection stays open for heartbeats)
        if !remaining_accounts.is_empty() {
            self.send_authenticated_message("connect", Some(remaining_accounts))
                .await?;
        }

        Ok(())
    }

    /// Get the next notification with automatic reconnection support.
    ///
    /// If `reconnect_config.enabled` is `true` (the default), this method will
    /// automatically attempt to reconnect when the connection drops. It will:
    ///
    /// 1. Emit a `Disconnected` notification when connection is lost
    /// 2. Wait according to exponential backoff strategy
    /// 3. Attempt to reconnect and restore subscriptions
    /// 4. Emit a `Reconnected` notification on success
    /// 5. Continue delivering notifications
    ///
    /// If reconnection fails after `max_attempts`, returns `None` to indicate
    /// the stream has ended.
    ///
    /// # Example
    ///
    /// ```ignore
    /// while let Some(result) = streamer.next().await {
    ///     match result? {
    ///         AccountNotification::Order(order) => {
    ///             println!("Order update: {:?}", order);
    ///         }
    ///         AccountNotification::Disconnected { reason } => {
    ///             // Auto-reconnect will be attempted if enabled
    ///             println!("Connection lost: {}, reconnecting...", reason);
    ///         }
    ///         AccountNotification::Reconnected { accounts_restored } => {
    ///             println!("Reconnected with {} accounts", accounts_restored);
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub async fn next(&mut self) -> Option<Result<AccountNotification>> {
        loop {
            // If we're in the middle of auto-reconnecting, attempt reconnection
            if self.auto_reconnecting {
                // Check if max attempts exceeded
                if self.reconnect_config.max_attempts_exceeded(self.reconnect_attempts) {
                    tracing::error!(
                        attempts = self.reconnect_attempts,
                        max_attempts = self.reconnect_config.max_attempts,
                        "Auto-reconnection failed: maximum attempts exceeded"
                    );
                    self.auto_reconnecting = false;
                    return None; // Stream has ended
                }

                // Calculate backoff delay
                let backoff = self.reconnect_config.backoff_for_attempt(self.reconnect_attempts);
                tracing::info!(
                    attempt = self.reconnect_attempts + 1,
                    backoff_ms = backoff.as_millis() as u64,
                    "Attempting auto-reconnection"
                );

                // Wait for backoff period
                tokio::time::sleep(backoff).await;

                // Attempt reconnection
                match self.reconnect().await {
                    Ok(()) => {
                        // Reconnection successful
                        let accounts_restored = self.subscription_count().await;
                        self.reconnect_attempts = 0;
                        self.auto_reconnecting = false;

                        tracing::info!(
                            accounts_restored = accounts_restored,
                            "Auto-reconnection successful"
                        );

                        return Some(Ok(AccountNotification::Reconnected { accounts_restored }));
                    }
                    Err(e) => {
                        // Reconnection failed, increment attempt counter
                        self.reconnect_attempts += 1;
                        tracing::warn!(
                            attempt = self.reconnect_attempts,
                            error = %e,
                            "Auto-reconnection attempt failed"
                        );
                        // Loop continues to try again
                        continue;
                    }
                }
            }

            // Normal notification receiving
            match self.notification_rx.recv().await {
                Some(Ok(AccountNotification::Disconnected { reason })) => {
                    // Connection was lost
                    if self.reconnect_config.enabled {
                        // Start auto-reconnection on next call
                        self.auto_reconnecting = true;
                        tracing::warn!(
                            reason = %reason,
                            "Connection lost, auto-reconnection enabled"
                        );
                    }
                    // Always emit the Disconnected event first
                    return Some(Ok(AccountNotification::Disconnected { reason }));
                }
                Some(Ok(notification)) => {
                    // Reset reconnect attempts on successful message
                    self.reconnect_attempts = 0;
                    return Some(Ok(notification));
                }
                Some(Err(e)) => {
                    return Some(Err(e));
                }
                None => {
                    // Channel closed - connection ended
                    if self.reconnect_config.enabled && !self.auto_reconnecting {
                        // Unexpected channel close, trigger auto-reconnect
                        self.auto_reconnecting = true;
                        tracing::warn!("Notification channel closed unexpectedly, triggering auto-reconnect");
                        return Some(Ok(AccountNotification::Disconnected {
                            reason: "Channel closed unexpectedly".to_string(),
                        }));
                    }
                    return None;
                }
            }
        }
    }

    /// Get the next notification without automatic reconnection.
    ///
    /// Use this method if you want full control over reconnection behavior.
    /// Returns `None` when the connection ends.
    ///
    /// # Example
    ///
    /// ```ignore
    /// loop {
    ///     match streamer.next_raw().await {
    ///         Some(Ok(notification)) => {
    ///             // Handle notification
    ///         }
    ///         Some(Err(e)) => {
    ///             eprintln!("Error: {}", e);
    ///         }
    ///         None => {
    ///             // Connection ended, manually handle reconnection
    ///             streamer.reconnect().await?;
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn next_raw(&mut self) -> Option<Result<AccountNotification>> {
        self.notification_rx.recv().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        let mut write = self.write.write().await;
        write.close().await?;
        Ok(())
    }

    // ===== Subscription State Inspection Methods (Gap 2) =====

    /// Get a list of currently subscribed account numbers.
    ///
    /// This returns a snapshot of the current subscriptions. The actual
    /// server-side subscription state may differ if messages are in flight.
    ///
    /// # Example
    /// ```ignore
    /// let accounts = streamer.subscribed_accounts().await;
    /// println!("Subscribed to {} accounts", accounts.len());
    /// ```
    pub async fn subscribed_accounts(&self) -> Vec<AccountNumber> {
        let subs = self.subscriptions.read().await;
        subs.iter()
            .map(|s| AccountNumber::new(s))
            .collect()
    }

    /// Check if currently subscribed to a specific account.
    ///
    /// # Example
    /// ```ignore
    /// if streamer.is_subscribed(&account).await {
    ///     println!("Subscribed to {}", account);
    /// }
    /// ```
    pub async fn is_subscribed(&self, account: &AccountNumber) -> bool {
        let subs = self.subscriptions.read().await;
        subs.contains(account.as_str())
    }

    /// Get the count of active subscriptions.
    ///
    /// # Example
    /// ```ignore
    /// let count = streamer.subscription_count().await;
    /// println!("Subscribed to {} accounts", count);
    /// ```
    pub async fn subscription_count(&self) -> usize {
        let subs = self.subscriptions.read().await;
        subs.len()
    }

    /// Get subscribed accounts synchronously (non-blocking).
    ///
    /// This returns `None` if the lock cannot be acquired immediately.
    /// Prefer `subscribed_accounts()` in async contexts.
    ///
    /// # Example
    /// ```ignore
    /// if let Some(accounts) = streamer.try_subscribed_accounts() {
    ///     println!("Subscribed to {} accounts", accounts.len());
    /// }
    /// ```
    pub fn try_subscribed_accounts(&self) -> Option<Vec<AccountNumber>> {
        self.subscriptions.try_read().ok().map(|subs| {
            subs.iter()
                .map(|s| AccountNumber::new(s))
                .collect()
        })
    }

    /// Get the next request ID for message tracking.
    fn next_request_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Send an authenticated message with the standard TastyTrade format.
    ///
    /// All messages include: auth-token, action, request-id, source
    /// Optional: value (for subscriptions)
    async fn send_authenticated_message<T: serde::Serialize>(
        &self,
        action: &str,
        value: Option<T>,
    ) -> Result<()> {
        let auth_token = self.auth_token.read().await.clone();
        let request_id = self.next_request_id();
        let source = format!("tastytrade-rs/{}", env!("CARGO_PKG_VERSION"));

        let msg = if let Some(val) = value {
            serde_json::json!({
                "auth-token": auth_token,
                "action": action,
                "request-id": request_id,
                "source": source,
                "value": val
            })
        } else {
            serde_json::json!({
                "auth-token": auth_token,
                "action": action,
                "request-id": request_id,
                "source": source
            })
        };

        let mut write = self.write.write().await;
        write.send(Message::Text(msg.to_string())).await?;
        Ok(())
    }

    /// Background task that sends heartbeat messages every 15 seconds.
    async fn heartbeat_task(
        write: Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        auth_token: Arc<RwLock<String>>,
        connected: Arc<AtomicBool>,
        request_id: Arc<AtomicU64>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        let source = format!("tastytrade-rs/{}", env!("CARGO_PKG_VERSION"));

        loop {
            interval.tick().await;

            // Stop if disconnected
            if !connected.load(Ordering::SeqCst) {
                return;
            }

            // Build heartbeat message
            let token = auth_token.read().await.clone();
            let req_id = request_id.fetch_add(1, Ordering::SeqCst);

            let msg = serde_json::json!({
                "auth-token": token,
                "action": "heartbeat",
                "request-id": req_id,
                "source": source
            });

            // Send heartbeat
            let mut w = write.write().await;
            if w.send(Message::Text(msg.to_string())).await.is_err() {
                connected.store(false, Ordering::SeqCst);
                return;
            }
        }
    }

    async fn process_messages(
        mut read: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        notification_tx: mpsc::Sender<Result<AccountNotification>>,
        _subscriptions: Arc<RwLock<HashSet<String>>>,
        connected: Arc<AtomicBool>,
    ) {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(notification) = Self::parse_notification(&text) {
                        if notification_tx.send(Ok(notification)).await.is_err() {
                            connected.store(false, Ordering::SeqCst);
                            return;
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Ping handling is done at the stream level
                    let _ = data;
                }
                Ok(Message::Close(frame)) => {
                    // Mark as disconnected
                    connected.store(false, Ordering::SeqCst);

                    // Emit Disconnected notification before the error
                    let reason = frame
                        .map(|f| f.reason.to_string())
                        .unwrap_or_else(|| "Connection closed by server".to_string());

                    let _ = notification_tx
                        .send(Ok(AccountNotification::Disconnected {
                            reason: reason.clone(),
                        }))
                        .await;

                    // Also send the error for callers that expect Result-based handling
                    let _ = notification_tx.send(Err(Error::StreamDisconnected)).await;
                    return;
                }
                Err(e) => {
                    // Mark as disconnected
                    connected.store(false, Ordering::SeqCst);

                    // Emit Disconnected notification before the error
                    let reason = e.to_string();
                    let _ = notification_tx
                        .send(Ok(AccountNotification::Disconnected {
                            reason: reason.clone(),
                        }))
                        .await;

                    // Also send the error for callers that expect Result-based handling
                    let _ = notification_tx
                        .send(Err(Error::WebSocket(reason)))
                        .await;
                    return;
                }
                _ => {}
            }
        }

        // Mark as disconnected
        connected.store(false, Ordering::SeqCst);

        // Stream ended unexpectedly
        let _ = notification_tx
            .send(Ok(AccountNotification::Disconnected {
                reason: "Stream ended unexpectedly".to_string(),
            }))
            .await;
    }

    fn parse_notification(text: &str) -> Result<AccountNotification> {
        // Try to parse the JSON
        let json: serde_json::Value = serde_json::from_str(text)?;

        // Get the action type
        let action = json.get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("");

        // Check for status field (for connect responses)
        let status = json.get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("");

        match action {
            "heartbeat" => Ok(AccountNotification::Heartbeat),
            // Handle "connect" action with status "ok" as subscription confirmation
            "connect" if status == "ok" => {
                let accounts = json.get("value")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(AccountNotification::SubscriptionConfirmation { accounts })
            }
            // Handle "connect" action with status "error" as connection failure
            "connect" if status == "error" => {
                let message = json.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                tracing::error!(
                    message = message,
                    raw_json = %json,
                    "Account subscription failed - session may be expired or account unavailable"
                );
                Ok(AccountNotification::ConnectionWarning {
                    message: format!("Subscription failed: {}", message),
                })
            }
            "subscription-confirmation" => {
                let accounts = json.get("value")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(AccountNotification::SubscriptionConfirmation { accounts })
            }
            "order" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                match serde_json::from_value::<OrderNotification>(data.clone()) {
                    Ok(order) => Ok(AccountNotification::Order(order)),
                    Err(e) => {
                        tracing::error!(
                            action = "order",
                            error = %e,
                            raw_data = %data,
                            "Failed to deserialize order notification - potential data loss"
                        );
                        Ok(AccountNotification::ParseError {
                            action: "order".to_string(),
                            error: e.to_string(),
                            raw_data: data,
                        })
                    }
                }
            }
            "position" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                match serde_json::from_value::<PositionNotification>(data.clone()) {
                    Ok(position) => Ok(AccountNotification::Position(position)),
                    Err(e) => {
                        tracing::error!(
                            action = "position",
                            error = %e,
                            raw_data = %data,
                            "Failed to deserialize position notification - potential data loss"
                        );
                        Ok(AccountNotification::ParseError {
                            action: "position".to_string(),
                            error: e.to_string(),
                            raw_data: data,
                        })
                    }
                }
            }
            "balance" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                match serde_json::from_value::<BalanceNotification>(data.clone()) {
                    Ok(balance) => Ok(AccountNotification::Balance(balance)),
                    Err(e) => {
                        tracing::error!(
                            action = "balance",
                            error = %e,
                            raw_data = %data,
                            "Failed to deserialize balance notification - potential data loss"
                        );
                        Ok(AccountNotification::ParseError {
                            action: "balance".to_string(),
                            error: e.to_string(),
                            raw_data: data,
                        })
                    }
                }
            }
            "quote-alert" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                match serde_json::from_value::<QuoteAlertNotification>(data.clone()) {
                    Ok(alert) => Ok(AccountNotification::QuoteAlert(alert)),
                    Err(e) => {
                        tracing::error!(
                            action = "quote-alert",
                            error = %e,
                            raw_data = %data,
                            "Failed to deserialize quote alert notification - potential data loss"
                        );
                        Ok(AccountNotification::ParseError {
                            action: "quote-alert".to_string(),
                            error: e.to_string(),
                            raw_data: data,
                        })
                    }
                }
            }
            _ => {
                // Log unknown notification types for monitoring
                tracing::warn!(
                    action = action,
                    raw_json = %json,
                    "Received unknown notification type - consider updating the library"
                );
                Ok(AccountNotification::Unknown(json))
            }
        }
    }
}
