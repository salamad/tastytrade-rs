//! DXLink market data streaming.
//!
//! This module provides real-time market data streaming via the DXLink
//! WebSocket protocol. It implements the DXLink protocol with COMPACT
//! data format for efficient market data delivery.
//!
//! # COMPACT Format
//!
//! DXLink uses COMPACT format where event data is sent as positional arrays
//! rather than JSON objects with named fields. This significantly reduces
//! bandwidth and parsing overhead for high-frequency market data.
//!
//! # Example
//!
//! ```ignore
//! use tastytrade_rs::streaming::{DxLinkStreamer, Quote};
//!
//! let mut streamer = client.streaming().dxlink().await?;
//! streamer.subscribe::<Quote>(&["SPY", "AAPL"]).await?;
//!
//! while let Some(event) = streamer.next().await {
//!     match event? {
//!         DxEvent::Quote(quote) => println!("{}: bid={:?} ask={:?}",
//!             quote.event_symbol, quote.bid_price, quote.ask_price),
//!         _ => {}
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ClientInner;
use crate::streaming::account::ReconnectConfig;
use crate::{Error, Result};

mod compact;
mod events;

pub use compact::fields;
pub use events::*;

/// Keepalive interval in seconds.
///
/// Per DXLink protocol, the client must send KEEPALIVE every 30 seconds
/// to prevent the 60-second timeout from closing the connection.
const KEEPALIVE_INTERVAL_SECS: u64 = 30;

/// DXLink market data streamer.
///
/// Provides real-time market data via WebSocket connection to dxFeed.
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
/// use tastytrade_rs::streaming::{DxLinkStreamer, Quote, ReconnectConfig};
///
/// let mut streamer = client.streaming().dxlink().await?
///     .with_reconnect_config(ReconnectConfig::aggressive());
///
/// streamer.subscribe::<Quote>(&["SPY", "AAPL"]).await?;
///
/// while let Some(event) = streamer.next().await {
///     if let Err(e) = event {
///         if !streamer.is_connected() {
///             streamer.reconnect().await?;
///         }
///     }
/// }
/// ```
pub struct DxLinkStreamer {
    write: Arc<RwLock<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >>>,
    event_rx: mpsc::Receiver<Result<DxEvent>>,
    subscriptions: Arc<RwLock<HashMap<String, EventType>>>,
    channel_id: i32,
    /// Connection state for keepalive task coordination
    connected: Arc<AtomicBool>,
    /// Client reference for reconnection
    client: Arc<ClientInner>,
    /// Reconnection configuration
    reconnect_config: ReconnectConfig,
}

/// DXLink event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Bid/ask quotes
    Quote,
    /// Trade executions
    Trade,
    /// Option greeks
    Greeks,
    /// Daily summary (OHLC)
    Summary,
    /// Security profile
    Profile,
    /// Candlestick data
    Candle,
    /// Theoretical price
    TheoPrice,
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            EventType::Quote => "Quote",
            EventType::Trade => "Trade",
            EventType::Greeks => "Greeks",
            EventType::Summary => "Summary",
            EventType::Profile => "Profile",
            EventType::Candle => "Candle",
            EventType::TheoPrice => "TheoPrice",
        }
    }
}

/// Trait for DXLink event types.
pub trait DxEventTrait: Sized + for<'de> Deserialize<'de> {
    /// Get the event type.
    fn event_type() -> EventType;
}

/// Union of all DXLink event types.
#[derive(Debug, Clone)]
pub enum DxEvent {
    /// Quote event
    Quote(Quote),
    /// Trade event
    Trade(Trade),
    /// Greeks event
    Greeks(Greeks),
    /// Summary event
    Summary(Summary),
    /// Profile event
    Profile(Profile),
    /// Candle event
    Candle(Candle),
    /// Theoretical price event
    TheoPrice(TheoPrice),
}

/// Timeout for waiting for authentication responses.
const AUTH_TIMEOUT_SECS: u64 = 10;

impl DxLinkStreamer {
    /// Connect to the DXLink streaming service.
    pub(crate) async fn connect(client: Arc<ClientInner>) -> Result<Self> {
        // Get the API quote token
        let token = Self::get_quote_token(&client).await?;

        // Connect to WebSocket
        let url = client.session.environment().await.dxlink_url();
        let (ws_stream, _) = connect_async(url).await?;

        // Channel ID for the feed channel
        let channel_id = 1;

        // Perform full DXLink handshake (verifies AUTH_STATE and CHANNEL_OPENED)
        let (write, read) = Self::perform_handshake(ws_stream, &token, channel_id).await?;

        let write = Arc::new(RwLock::new(write));
        let subscriptions = Arc::new(RwLock::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(1024);

        // Start message processing task
        let write_clone = write.clone();
        let subs_clone = subscriptions.clone();
        let connected_clone = connected.clone();
        tokio::spawn(async move {
            Self::process_messages(read, event_tx, write_clone, subs_clone, connected_clone).await;
        });

        // Start keepalive task (sends KEEPALIVE every 30 seconds)
        let keepalive_write = write.clone();
        let keepalive_connected = connected.clone();
        tokio::spawn(async move {
            Self::keepalive_task(keepalive_write, keepalive_connected).await;
        });

        Ok(Self {
            write,
            event_rx,
            subscriptions,
            channel_id,
            connected,
            client,
            reconnect_config: ReconnectConfig::default(),
        })
    }

    /// Subscribe to events for the given symbols.
    ///
    /// Each symbol is sent as a separate entry in the subscription message,
    /// per DXLink protocol requirements.
    pub async fn subscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();

        // Build add array with one entry per symbol (per DXLink protocol)
        let add: Vec<_> = symbols
            .iter()
            .map(|s| {
                serde_json::json!({
                    "type": event_type.as_str(),
                    "symbol": s
                })
            })
            .collect();

        let msg = serde_json::json!({
            "type": "FEED_SUBSCRIPTION",
            "channel": self.channel_id,
            "add": add
        });

        self.send_message(&msg).await?;

        // Track subscriptions
        let mut subs = self.subscriptions.write().await;
        for symbol in symbols {
            subs.insert(symbol.to_string(), event_type);
        }

        Ok(())
    }

    /// Unsubscribe from events for the given symbols.
    ///
    /// Each symbol is sent as a separate entry in the unsubscribe message,
    /// per DXLink protocol requirements.
    pub async fn unsubscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();

        // Build remove array with one entry per symbol (per DXLink protocol)
        let remove: Vec<_> = symbols
            .iter()
            .map(|s| {
                serde_json::json!({
                    "type": event_type.as_str(),
                    "symbol": s
                })
            })
            .collect();

        let msg = serde_json::json!({
            "type": "FEED_SUBSCRIPTION",
            "channel": self.channel_id,
            "remove": remove
        });

        self.send_message(&msg).await?;

        // Remove from tracking
        let mut subs = self.subscriptions.write().await;
        for symbol in symbols {
            subs.remove(*symbol);
        }

        Ok(())
    }

    /// Get the next event.
    pub async fn next(&mut self) -> Option<Result<DxEvent>> {
        self.event_rx.recv().await
    }

    /// Check if the WebSocket connection is currently active.
    ///
    /// Returns `true` if the connection is established and healthy.
    /// Note that this is an atomic check and the state may change immediately after.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        // Mark as disconnected to stop keepalive task
        self.connected.store(false, Ordering::SeqCst);
        let mut write = self.write.write().await;
        write.close().await?;
        Ok(())
    }

    /// Get the current reconnection configuration.
    pub fn reconnect_config(&self) -> &ReconnectConfig {
        &self.reconnect_config
    }

    /// Configure reconnection behavior.
    ///
    /// This is a builder-style method for configuring reconnection.
    /// Call this immediately after connecting.
    ///
    /// # Example
    /// ```ignore
    /// let streamer = client.streaming().dxlink().await?
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
    /// 3. Re-authenticate with DXLink
    /// 4. Re-subscribe to all previously subscribed symbols
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
        let subscriptions: Vec<(String, EventType)> = {
            let subs = self.subscriptions.read().await;
            subs.iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect()
        };

        // Get a fresh quote token
        let token = Self::get_quote_token(&self.client).await?;

        // Connect to WebSocket
        let url = self.client.session.environment().await.dxlink_url();
        let (ws_stream, _) = connect_async(url).await?;

        // Perform handshake
        let (write, read) = Self::perform_handshake(ws_stream, &token, self.channel_id).await?;

        // Update write handle
        *self.write.write().await = write;

        // Create new event channel
        let (event_tx, event_rx) = mpsc::channel(1024);
        self.event_rx = event_rx;

        // Mark as connected
        self.connected.store(true, Ordering::SeqCst);

        // Start message processing task
        let write_clone = self.write.clone();
        let subs_clone = self.subscriptions.clone();
        let connected_clone = self.connected.clone();
        tokio::spawn(async move {
            Self::process_messages(read, event_tx, write_clone, subs_clone, connected_clone).await;
        });

        // Start new keepalive task
        let keepalive_write = self.write.clone();
        let keepalive_connected = self.connected.clone();
        tokio::spawn(async move {
            Self::keepalive_task(keepalive_write, keepalive_connected).await;
        });

        // Re-subscribe to all symbols, grouped by event type
        let mut by_type: HashMap<EventType, Vec<String>> = HashMap::new();
        for (symbol, event_type) in subscriptions {
            by_type.entry(event_type).or_default().push(symbol);
        }

        for (event_type, symbols) in by_type {
            // Build add array with one entry per symbol
            let add: Vec<_> = symbols
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "type": event_type.as_str(),
                        "symbol": s
                    })
                })
                .collect();

            let msg = serde_json::json!({
                "type": "FEED_SUBSCRIPTION",
                "channel": self.channel_id,
                "add": add
            });

            self.send_message(&msg).await?;
        }

        Ok(())
    }

    /// Get a list of currently subscribed symbols with their event types.
    ///
    /// This returns a snapshot of the current subscriptions.
    ///
    /// # Example
    /// ```ignore
    /// let subs = streamer.subscribed_symbols().await;
    /// for (symbol, event_type) in subs {
    ///     println!("{}: {:?}", symbol, event_type);
    /// }
    /// ```
    pub async fn subscribed_symbols(&self) -> Vec<(String, EventType)> {
        let subs = self.subscriptions.read().await;
        subs.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Get the count of active subscriptions.
    pub async fn subscription_count(&self) -> usize {
        let subs = self.subscriptions.read().await;
        subs.len()
    }

    async fn get_quote_token(client: &ClientInner) -> Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct TokenData {
            token: String,
            #[allow(dead_code)]
            dxlink_url: String,
        }

        let data: TokenData = client.get("/api-quote-tokens").await?;
        Ok(data.token)
    }

    /// Default aggregation period in seconds for FEED_SETUP.
    /// 0.1 seconds = 100ms, which provides a good balance between
    /// latency and bandwidth.
    const DEFAULT_AGGREGATION_PERIOD: f64 = 0.1;

    /// DXLink protocol version.
    const DXLINK_VERSION: &'static str = "0.1-DXF-JS/0.3.0";

    /// Perform the full DXLink handshake with response verification.
    ///
    /// This implements the proper DXLink protocol sequence:
    /// 1. SETUP → wait for SETUP response
    /// 2. AUTH → wait for AUTH_STATE: AUTHORIZED
    /// 3. CHANNEL_REQUEST → wait for CHANNEL_OPENED
    /// 4. FEED_SETUP → wait for FEED_CONFIG
    ///
    /// Returns the split WebSocket stream after successful authentication.
    async fn perform_handshake(
        mut ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        token: &str,
        channel_id: i32,
    ) -> Result<(
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    )> {
        use tokio::time::timeout;

        // Step 1: Send SETUP message
        let setup = serde_json::json!({
            "type": "SETUP",
            "channel": 0,
            "keepaliveTimeout": 60,
            "acceptKeepaliveTimeout": 60,
            "version": Self::DXLINK_VERSION
        });
        ws_stream.send(Message::Text(setup.to_string())).await?;

        // Step 2: Send AUTH message
        let auth = serde_json::json!({
            "type": "AUTH",
            "channel": 0,
            "token": token
        });
        ws_stream.send(Message::Text(auth.to_string())).await?;

        // Step 3: Wait for AUTH_STATE: AUTHORIZED
        let auth_timeout = Duration::from_secs(AUTH_TIMEOUT_SECS);
        let mut authorized = false;

        loop {
            let msg = timeout(auth_timeout, ws_stream.next())
                .await
                .map_err(|_| Error::Timeout("Waiting for AUTH_STATE".to_string()))?;

            match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "AUTH_STATE" => {
                                    if let Some(state) = json.get("state").and_then(|s| s.as_str())
                                    {
                                        if state == "AUTHORIZED" {
                                            authorized = true;
                                            break;
                                        } else if state == "UNAUTHORIZED" {
                                            // Initial state, continue waiting
                                            continue;
                                        } else {
                                            return Err(Error::Authentication(format!(
                                                "Unexpected AUTH_STATE: {}",
                                                state
                                            )));
                                        }
                                    }
                                }
                                "SETUP" | "KEEPALIVE" => {
                                    // Expected responses, continue
                                    continue;
                                }
                                "ERROR" => {
                                    let error_msg = json
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error");
                                    return Err(Error::Authentication(error_msg.to_string()));
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(data))) => {
                    ws_stream.send(Message::Pong(data)).await?;
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(Error::StreamDisconnected);
                }
                Some(Err(e)) => {
                    return Err(Error::WebSocket(e.to_string()));
                }
                None => {
                    return Err(Error::StreamDisconnected);
                }
                _ => continue,
            }
        }

        if !authorized {
            return Err(Error::Authentication(
                "Failed to receive AUTH_STATE: AUTHORIZED".to_string(),
            ));
        }

        // Step 4: Send CHANNEL_REQUEST
        let channel_request = serde_json::json!({
            "type": "CHANNEL_REQUEST",
            "channel": channel_id,
            "service": "FEED",
            "parameters": {
                "contract": "AUTO"
            }
        });
        ws_stream
            .send(Message::Text(channel_request.to_string()))
            .await?;

        // Step 5: Wait for CHANNEL_OPENED
        let mut channel_opened = false;

        loop {
            let msg = timeout(auth_timeout, ws_stream.next())
                .await
                .map_err(|_| Error::Timeout("Waiting for CHANNEL_OPENED".to_string()))?;

            match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "CHANNEL_OPENED" => {
                                    // Verify it's for our channel
                                    if json.get("channel").and_then(|c| c.as_i64())
                                        == Some(channel_id as i64)
                                    {
                                        channel_opened = true;
                                        break;
                                    }
                                }
                                "KEEPALIVE" => {
                                    // Respond to keepalive
                                    let response = serde_json::json!({
                                        "type": "KEEPALIVE",
                                        "channel": 0
                                    });
                                    ws_stream.send(Message::Text(response.to_string())).await?;
                                }
                                "ERROR" => {
                                    let error_msg = json
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error");
                                    return Err(Error::Authentication(error_msg.to_string()));
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(data))) => {
                    ws_stream.send(Message::Pong(data)).await?;
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(Error::StreamDisconnected);
                }
                Some(Err(e)) => {
                    return Err(Error::WebSocket(e.to_string()));
                }
                None => {
                    return Err(Error::StreamDisconnected);
                }
                _ => continue,
            }
        }

        if !channel_opened {
            return Err(Error::Authentication(
                "Failed to receive CHANNEL_OPENED".to_string(),
            ));
        }

        // Step 6: Send FEED_SETUP
        let feed_setup = serde_json::json!({
            "type": "FEED_SETUP",
            "channel": channel_id,
            "acceptAggregationPeriod": Self::DEFAULT_AGGREGATION_PERIOD,
            "acceptDataFormat": "COMPACT",
            "acceptEventFields": compact::get_accept_event_fields()
        });
        ws_stream.send(Message::Text(feed_setup.to_string())).await?;

        // Step 7: Wait for FEED_CONFIG (optional but good for verification)
        loop {
            let msg = timeout(auth_timeout, ws_stream.next())
                .await
                .map_err(|_| Error::Timeout("Waiting for FEED_CONFIG".to_string()))?;

            match msg {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "FEED_CONFIG" => {
                                    // Feed is configured, we're done
                                    break;
                                }
                                "KEEPALIVE" => {
                                    let response = serde_json::json!({
                                        "type": "KEEPALIVE",
                                        "channel": 0
                                    });
                                    ws_stream.send(Message::Text(response.to_string())).await?;
                                }
                                "ERROR" => {
                                    let error_msg = json
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error");
                                    return Err(Error::Authentication(error_msg.to_string()));
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(data))) => {
                    ws_stream.send(Message::Pong(data)).await?;
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(Error::StreamDisconnected);
                }
                Some(Err(e)) => {
                    return Err(Error::WebSocket(e.to_string()));
                }
                None => {
                    return Err(Error::StreamDisconnected);
                }
                _ => continue,
            }
        }

        // Split the stream for async processing
        Ok(ws_stream.split())
    }

    async fn send_message(&self, msg: &serde_json::Value) -> Result<()> {
        let mut write = self.write.write().await;
        write.send(Message::Text(msg.to_string())).await?;
        Ok(())
    }

    /// Background task that sends KEEPALIVE messages every 30 seconds.
    ///
    /// Per DXLink protocol, the client must proactively send KEEPALIVE
    /// to prevent the 60-second timeout from closing the connection.
    async fn keepalive_task(
        write: Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        connected: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(KEEPALIVE_INTERVAL_SECS));

        loop {
            interval.tick().await;

            // Stop if disconnected
            if !connected.load(Ordering::SeqCst) {
                return;
            }

            // Send KEEPALIVE message
            let msg = serde_json::json!({
                "type": "KEEPALIVE",
                "channel": 0
            });

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
        event_tx: mpsc::Sender<Result<DxEvent>>,
        write: Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        _subscriptions: Arc<RwLock<HashMap<String, EventType>>>,
        connected: Arc<AtomicBool>,
    ) {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        // Handle different message types
                        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                            match msg_type {
                                "FEED_DATA" => {
                                    if let Some(events) = Self::parse_feed_data(&json) {
                                        for event in events {
                                            if event_tx.send(Ok(event)).await.is_err() {
                                                connected.store(false, Ordering::SeqCst);
                                                return;
                                            }
                                        }
                                    }
                                }
                                "KEEPALIVE" => {
                                    // Server sent a keepalive, respond to it
                                    let response = serde_json::json!({
                                        "type": "KEEPALIVE",
                                        "channel": 0
                                    });
                                    let mut w = write.write().await;
                                    if w.send(Message::Text(response.to_string())).await.is_err() {
                                        connected.store(false, Ordering::SeqCst);
                                        return;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    let mut w = write.write().await;
                    let _ = w.send(Message::Pong(data)).await;
                }
                Ok(Message::Close(_)) => {
                    connected.store(false, Ordering::SeqCst);
                    let _ = event_tx.send(Err(Error::StreamDisconnected)).await;
                    return;
                }
                Err(e) => {
                    connected.store(false, Ordering::SeqCst);
                    let _ = event_tx.send(Err(Error::WebSocket(e.to_string()))).await;
                    return;
                }
                _ => {}
            }
        }

        // Stream ended unexpectedly
        connected.store(false, Ordering::SeqCst);
    }

    /// Parse FEED_DATA message using COMPACT format parser.
    ///
    /// COMPACT format structure:
    /// ```json
    /// {"type":"FEED_DATA","channel":1,"data":["Quote",["Quote","SPY",450.25,450.30,100,200]]}
    /// ```
    fn parse_feed_data(json: &serde_json::Value) -> Option<Vec<DxEvent>> {
        let data = json.get("data")?;
        let events = compact::parse_compact_feed_data(data);

        if events.is_empty() {
            None
        } else {
            Some(events)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::Quote.as_str(), "Quote");
        assert_eq!(EventType::Trade.as_str(), "Trade");
        assert_eq!(EventType::Greeks.as_str(), "Greeks");
        assert_eq!(EventType::Summary.as_str(), "Summary");
        assert_eq!(EventType::Profile.as_str(), "Profile");
        assert_eq!(EventType::Candle.as_str(), "Candle");
        assert_eq!(EventType::TheoPrice.as_str(), "TheoPrice");
    }

    #[test]
    fn test_parse_feed_data_compact_quote() {
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1,
            "data": ["Quote", ["Quote", "SPY", 450.25, 450.30, 100, 200]]
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_some());
        let events = events.unwrap();
        assert_eq!(events.len(), 1);

        if let DxEvent::Quote(quote) = &events[0] {
            assert_eq!(quote.event_symbol, "SPY");
        } else {
            panic!("Expected Quote event");
        }
    }

    #[test]
    fn test_parse_feed_data_compact_trade() {
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1,
            "data": ["Trade", ["Trade", "AAPL", 175.50, 1000000, 100]]
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_some());
        let events = events.unwrap();
        assert_eq!(events.len(), 1);

        if let DxEvent::Trade(trade) = &events[0] {
            assert_eq!(trade.event_symbol, "AAPL");
        } else {
            panic!("Expected Trade event");
        }
    }

    #[test]
    fn test_parse_feed_data_compact_greeks() {
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1,
            "data": ["Greeks", ["Greeks", ".SPY230120C450", 0.25, 0.55, 0.02, -0.05, 0.01, 0.15]]
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_some());
        let events = events.unwrap();
        assert_eq!(events.len(), 1);

        if let DxEvent::Greeks(greeks) = &events[0] {
            assert_eq!(greeks.event_symbol, ".SPY230120C450");
        } else {
            panic!("Expected Greeks event");
        }
    }

    #[test]
    fn test_parse_feed_data_multiple_events() {
        // Multiple events in flattened array format
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1,
            "data": ["Quote", [
                "Quote", "SPY", 450.25, 450.30, 100, 200,
                "Quote", "AAPL", 175.00, 175.05, 50, 75
            ]]
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_some());
        let events = events.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_parse_feed_data_empty() {
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1,
            "data": []
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_none());
    }

    #[test]
    fn test_parse_feed_data_missing_data() {
        let json = serde_json::json!({
            "type": "FEED_DATA",
            "channel": 1
        });

        let events = DxLinkStreamer::parse_feed_data(&json);
        assert!(events.is_none());
    }
}
