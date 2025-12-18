//! Account streaming for real-time account updates.
//!
//! This module provides real-time account notifications via WebSocket,
//! including order status changes, position updates, and balance changes.

use std::collections::HashSet;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ClientInner;
use crate::models::AccountNumber;
use crate::{Error, Result};

mod notifications;
pub use notifications::*;

/// Account activity streamer.
///
/// Provides real-time notifications for account activity including:
/// - Order status changes
/// - Position updates
/// - Balance changes
/// - Account alerts
pub struct AccountStreamer {
    write: Arc<RwLock<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >>>,
    notification_rx: mpsc::Receiver<Result<AccountNotification>>,
    subscriptions: Arc<RwLock<HashSet<String>>>,
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
        // Get the streamer token
        let token = Self::get_streamer_token(&client).await?;

        // Connect to WebSocket
        let url = client.session.environment().await.account_streamer_url();
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        let write = Arc::new(RwLock::new(write));
        let subscriptions = Arc::new(RwLock::new(HashSet::new()));

        // Create notification channel
        let (notification_tx, notification_rx) = mpsc::channel(1024);

        // Authenticate
        Self::authenticate(&write, &token).await?;

        // Start message processing task
        let subs_clone = subscriptions.clone();
        tokio::spawn(async move {
            Self::process_messages(read, notification_tx, subs_clone).await;
        });

        Ok(Self {
            write,
            notification_rx,
            subscriptions,
        })
    }

    /// Subscribe to account updates.
    ///
    /// This will receive notifications for order changes, position updates,
    /// and balance changes for the specified account.
    pub async fn subscribe(&mut self, account_number: &AccountNumber) -> Result<()> {
        let msg = serde_json::json!({
            "action": "account-subscribe",
            "value": [account_number.as_str()]
        });

        self.send_message(&msg).await?;

        let mut subs = self.subscriptions.write().await;
        subs.insert(account_number.to_string());

        Ok(())
    }

    /// Subscribe to quote alerts.
    ///
    /// Receive notifications when price alerts are triggered.
    pub async fn subscribe_quote_alerts(&mut self) -> Result<()> {
        let msg = serde_json::json!({
            "action": "quote-alerts-subscribe"
        });

        self.send_message(&msg).await
    }

    /// Subscribe to user-level messages.
    ///
    /// Receive notifications about account-level events.
    pub async fn subscribe_user_messages(&mut self, external_id: &str) -> Result<()> {
        let msg = serde_json::json!({
            "action": "user-message-subscribe",
            "value": [external_id]
        });

        self.send_message(&msg).await
    }

    /// Unsubscribe from account updates.
    pub async fn unsubscribe(&mut self, account_number: &AccountNumber) -> Result<()> {
        let msg = serde_json::json!({
            "action": "account-unsubscribe",
            "value": [account_number.as_str()]
        });

        self.send_message(&msg).await?;

        let mut subs = self.subscriptions.write().await;
        subs.remove(account_number.as_str());

        Ok(())
    }

    /// Get the next notification.
    pub async fn next(&mut self) -> Option<Result<AccountNotification>> {
        self.notification_rx.recv().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        let mut write = self.write.write().await;
        write.close().await?;
        Ok(())
    }

    async fn get_streamer_token(client: &ClientInner) -> Result<String> {
        #[derive(Deserialize)]
        struct TokenData {
            token: String,
            #[serde(rename = "streamer-url")]
            #[allow(dead_code)]
            streamer_url: Option<String>,
        }

        #[derive(Deserialize)]
        struct Response {
            data: TokenData,
        }

        let response: Response = client.get("/api-quote-tokens").await?;
        Ok(response.data.token)
    }

    async fn authenticate(
        write: &Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        token: &str,
    ) -> Result<()> {
        let auth = serde_json::json!({
            "auth-token": token
        });

        let mut w = write.write().await;
        w.send(Message::Text(auth.to_string())).await?;

        Ok(())
    }

    async fn send_message(&self, msg: &serde_json::Value) -> Result<()> {
        let mut write = self.write.write().await;
        write.send(Message::Text(msg.to_string())).await?;
        Ok(())
    }

    async fn process_messages(
        mut read: futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        notification_tx: mpsc::Sender<Result<AccountNotification>>,
        _subscriptions: Arc<RwLock<HashSet<String>>>,
    ) {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(notification) = Self::parse_notification(&text) {
                        if notification_tx.send(Ok(notification)).await.is_err() {
                            return;
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Ping handling is done at the stream level
                    let _ = data;
                }
                Ok(Message::Close(_)) => {
                    let _ = notification_tx.send(Err(Error::StreamDisconnected)).await;
                    return;
                }
                Err(e) => {
                    let _ = notification_tx.send(Err(Error::WebSocket(e.to_string()))).await;
                    return;
                }
                _ => {}
            }
        }
    }

    fn parse_notification(text: &str) -> Result<AccountNotification> {
        // Try to parse the JSON
        let json: serde_json::Value = serde_json::from_str(text)?;

        // Get the action type
        let action = json.get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("");

        match action {
            "heartbeat" => Ok(AccountNotification::Heartbeat),
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
                Ok(AccountNotification::Order(
                    serde_json::from_value(data).unwrap_or_default()
                ))
            }
            "position" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                Ok(AccountNotification::Position(
                    serde_json::from_value(data).unwrap_or_default()
                ))
            }
            "balance" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                Ok(AccountNotification::Balance(
                    serde_json::from_value(data).unwrap_or_default()
                ))
            }
            "quote-alert" => {
                let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
                Ok(AccountNotification::QuoteAlert(
                    serde_json::from_value(data).unwrap_or_default()
                ))
            }
            _ => {
                // Return raw notification for unknown types
                Ok(AccountNotification::Unknown(json))
            }
        }
    }
}
