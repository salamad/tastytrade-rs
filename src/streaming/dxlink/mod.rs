//! DXLink market data streaming.
//!
//! This module provides real-time market data streaming via the DXLink
//! WebSocket protocol.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::client::ClientInner;
use crate::{Error, Result};

mod events;
pub use events::*;

/// DXLink market data streamer.
///
/// Provides real-time market data via WebSocket connection to dxFeed.
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

impl DxLinkStreamer {
    /// Connect to the DXLink streaming service.
    pub(crate) async fn connect(client: Arc<ClientInner>) -> Result<Self> {
        // Get the API quote token
        let token = Self::get_quote_token(&client).await?;

        // Connect to WebSocket
        let url = client.session.environment().await.dxlink_url();
        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        let write = Arc::new(RwLock::new(write));
        let subscriptions = Arc::new(RwLock::new(HashMap::new()));

        // Create event channel
        let (event_tx, event_rx) = mpsc::channel(1024);

        // Authenticate
        Self::authenticate(&write, &token).await?;

        // Start message processing task
        let write_clone = write.clone();
        let subs_clone = subscriptions.clone();
        tokio::spawn(async move {
            Self::process_messages(read, event_tx, write_clone, subs_clone).await;
        });

        Ok(Self {
            write,
            event_rx,
            subscriptions,
            channel_id: 1,
        })
    }

    /// Subscribe to events for the given symbols.
    pub async fn subscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();

        // Send subscription message
        let msg = serde_json::json!({
            "type": "FEED_SUBSCRIPTION",
            "channel": self.channel_id,
            "add": [{
                "type": event_type.as_str(),
                "symbol": symbols,
            }]
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
    pub async fn unsubscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();

        let msg = serde_json::json!({
            "type": "FEED_SUBSCRIPTION",
            "channel": self.channel_id,
            "remove": [{
                "type": event_type.as_str(),
                "symbol": symbols,
            }]
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

    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        let mut write = self.write.write().await;
        write.close().await?;
        Ok(())
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

    async fn authenticate(
        write: &Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        token: &str,
    ) -> Result<()> {
        // Send SETUP message
        let setup = serde_json::json!({
            "type": "SETUP",
            "channel": 0,
            "keepaliveTimeout": 60,
            "acceptKeepaliveTimeout": 60,
            "version": "0.1-js/1.0.0"
        });

        let mut w = write.write().await;
        w.send(Message::Text(setup.to_string())).await?;

        // Send AUTH message
        let auth = serde_json::json!({
            "type": "AUTH",
            "channel": 0,
            "token": token
        });
        w.send(Message::Text(auth.to_string())).await?;

        // Setup channel
        let channel_request = serde_json::json!({
            "type": "CHANNEL_REQUEST",
            "channel": 1,
            "service": "FEED",
            "parameters": {
                "contract": "AUTO"
            }
        });
        w.send(Message::Text(channel_request.to_string())).await?;

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
        event_tx: mpsc::Sender<Result<DxEvent>>,
        write: Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        _subscriptions: Arc<RwLock<HashMap<String, EventType>>>,
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
                                                return;
                                            }
                                        }
                                    }
                                }
                                "KEEPALIVE" => {
                                    // Respond to keepalive
                                    let response = serde_json::json!({
                                        "type": "KEEPALIVE",
                                        "channel": 0
                                    });
                                    let mut w = write.write().await;
                                    let _ = w.send(Message::Text(response.to_string())).await;
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
                    let _ = event_tx.send(Err(Error::StreamDisconnected)).await;
                    return;
                }
                Err(e) => {
                    let _ = event_tx.send(Err(Error::WebSocket(e.to_string()))).await;
                    return;
                }
                _ => {}
            }
        }
    }

    fn parse_feed_data(json: &serde_json::Value) -> Option<Vec<DxEvent>> {
        let data = json.get("data")?;
        let mut events = Vec::new();

        if let Some(arr) = data.as_array() {
            for item in arr {
                if let Some(event_type) = item.get("eventType").and_then(|t| t.as_str()) {
                    match event_type {
                        "Quote" => {
                            if let Ok(quote) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Quote(quote));
                            }
                        }
                        "Trade" => {
                            if let Ok(trade) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Trade(trade));
                            }
                        }
                        "Greeks" => {
                            if let Ok(greeks) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Greeks(greeks));
                            }
                        }
                        "Summary" => {
                            if let Ok(summary) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Summary(summary));
                            }
                        }
                        "Profile" => {
                            if let Ok(profile) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Profile(profile));
                            }
                        }
                        "Candle" => {
                            if let Ok(candle) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::Candle(candle));
                            }
                        }
                        "TheoPrice" => {
                            if let Ok(theo) = serde_json::from_value(item.clone()) {
                                events.push(DxEvent::TheoPrice(theo));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if events.is_empty() {
            None
        } else {
            Some(events)
        }
    }
}
