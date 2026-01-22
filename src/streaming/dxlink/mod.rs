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
//! # Automatic Quote Token Refresh
//!
//! TastyTrade quote tokens expire after 24 hours. By default, the streamer
//! automatically reconnects at 23 hours to obtain a fresh token before
//! expiration. This ensures uninterrupted market data for long-running
//! applications without requiring manual intervention.
//!
//! The automatic refresh behavior can be customized or disabled via
//! `DxLinkConfig`:
//!
//! ```ignore
//! use tastytrade_rs::streaming::DxLinkConfig;
//!
//! // Custom refresh interval (22 hours)
//! let config = DxLinkConfig::default()
//!     .with_quote_token_refresh_hours(22);
//!
//! // Disable automatic refresh
//! let config = DxLinkConfig::default()
//!     .with_auto_refresh_quote_token(false);
//! ```
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

use chrono::{DateTime, Utc};
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
/// # Automatic Quote Token Refresh
///
/// Quote tokens expire after 24 hours. By default, the streamer automatically
/// reconnects at 23 hours to obtain a fresh token. This happens transparently
/// during the `next()` call, ensuring uninterrupted market data streaming for
/// long-running applications.
///
/// Monitor token age with `quote_token_obtained_at()` and check if refresh
/// is needed with `needs_quote_token_refresh()`.
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
/// // Automatic token refresh happens transparently
/// while let Some(event) = streamer.next().await {
///     match event? {
///         DxEvent::Quote(quote) => println!("{:?}", quote),
///         _ => {}
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
    /// Channel states for multi-channel architecture
    channel_states: Arc<RwLock<HashMap<i32, ChannelState>>>,
    /// Connection state for keepalive task coordination
    connected: Arc<AtomicBool>,
    /// Client reference for reconnection
    client: Arc<ClientInner>,
    /// Reconnection configuration
    reconnect_config: ReconnectConfig,
    /// DXLink configuration
    config: DxLinkConfig,
    /// Timestamp when the quote token was obtained (for automatic refresh)
    quote_token_obtained_at: Arc<RwLock<DateTime<Utc>>>,
    /// Flag indicating quote token refresh is needed
    needs_token_refresh: Arc<AtomicBool>,
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
    /// Time and sales (individual trade details)
    TimeAndSale,
    /// Extended trading hours trades
    TradeETH,
    /// Underlying security information
    Underlying,
}

impl EventType {
    /// Get the string representation for the DXLink protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Quote => "Quote",
            EventType::Trade => "Trade",
            EventType::Greeks => "Greeks",
            EventType::Summary => "Summary",
            EventType::Profile => "Profile",
            EventType::Candle => "Candle",
            EventType::TheoPrice => "TheoPrice",
            EventType::TimeAndSale => "TimeAndSale",
            EventType::TradeETH => "TradeETH",
            EventType::Underlying => "Underlying",
        }
    }

    /// Get the dedicated channel ID for this event type.
    ///
    /// Per DXLink protocol best practices, each event type uses a separate
    /// channel to allow independent configuration and management.
    pub fn channel_id(&self) -> i32 {
        match self {
            EventType::Candle => 1,
            EventType::Greeks => 3,
            EventType::Profile => 5,
            EventType::Quote => 7,
            EventType::Summary => 9,
            EventType::TheoPrice => 11,
            EventType::TimeAndSale => 13,
            EventType::Trade => 15,
            EventType::TradeETH => 16,
            EventType::Underlying => 17,
        }
    }

    /// Get all event types.
    pub fn all() -> &'static [EventType] {
        &[
            EventType::Quote,
            EventType::Trade,
            EventType::Greeks,
            EventType::Summary,
            EventType::Profile,
            EventType::Candle,
            EventType::TheoPrice,
            EventType::TimeAndSale,
            EventType::TradeETH,
            EventType::Underlying,
        ]
    }
}

/// Channel state for tracking DXLink channel lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// Channel has not been opened
    Closed,
    /// Channel open request sent, waiting for CHANNEL_OPENED
    Opening,
    /// Channel is open and ready for subscriptions
    Opened,
    /// Channel close request sent
    Closing,
}

/// Candle period specification for candle subscriptions.
///
/// Candle symbols require a period suffix like `{=1d}` for daily candles.
/// This enum provides type-safe period specifications.
///
/// # Example
/// ```ignore
/// // Subscribe to daily candles
/// streamer.subscribe_candles(&["AAPL"], CandlePeriod::Day).await?;
///
/// // Subscribe to 5-minute candles
/// streamer.subscribe_candles(&["SPY"], CandlePeriod::Minutes(5)).await?;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CandlePeriod {
    /// Tick-based candles
    Tick,
    /// Second-based candles (1 second)
    Second,
    /// Minute-based candles (default: 1 minute)
    Minute,
    /// Custom minute-based candles (e.g., 5, 15, 30 minutes)
    Minutes(u32),
    /// Hourly candles
    Hour,
    /// Custom hour-based candles (e.g., 4 hours)
    Hours(u32),
    /// Daily candles
    Day,
    /// Weekly candles
    Week,
    /// Monthly candles
    Month,
    /// Yearly candles
    Year,
}

impl CandlePeriod {
    /// Get the string representation for candle symbol suffix.
    ///
    /// Returns the period string without the curly braces.
    pub fn as_str(&self) -> String {
        match self {
            CandlePeriod::Tick => "=t".to_string(),
            CandlePeriod::Second => "=s".to_string(),
            CandlePeriod::Minute => "=m".to_string(),
            CandlePeriod::Minutes(n) => format!("={}m", n),
            CandlePeriod::Hour => "=h".to_string(),
            CandlePeriod::Hours(n) => format!("={}h", n),
            CandlePeriod::Day => "=d".to_string(),
            CandlePeriod::Week => "=w".to_string(),
            CandlePeriod::Month => "=mo".to_string(),
            CandlePeriod::Year => "=y".to_string(),
        }
    }
}

/// Configuration options for DXLink streaming.
///
/// This struct provides various settings to customize the DXLink connection
/// behavior including timeouts, aggregation periods, buffer sizes, and
/// automatic quote token refresh.
///
/// # Automatic Quote Token Refresh
///
/// Quote tokens expire after 24 hours. By default, the streamer proactively
/// reconnects at 23 hours to obtain a fresh token before expiration.
/// This behavior can be disabled or customized.
///
/// # Example
/// ```ignore
/// let config = DxLinkConfig::default()
///     .with_aggregation_period(0.5)  // 500ms aggregation
///     .with_keepalive_interval_secs(20)
///     .with_quote_token_refresh_hours(22);  // Refresh at 22 hours
///
/// let streamer = client.streaming()
///     .dxlink_with_config(config).await?;
/// ```
#[derive(Debug, Clone)]
pub struct DxLinkConfig {
    /// Aggregation period in seconds for FEED_SETUP.
    /// Lower values provide more real-time data but increase bandwidth.
    /// Default: 0.1 (100ms)
    pub aggregation_period: f64,

    /// Keepalive interval in seconds.
    /// Must be less than 60 seconds (the server timeout).
    /// Default: 30 seconds
    pub keepalive_interval_secs: u64,

    /// Authentication timeout in seconds.
    /// How long to wait for AUTH_STATE: AUTHORIZED response.
    /// Default: 10 seconds
    pub auth_timeout_secs: u64,

    /// Channel open timeout in seconds.
    /// How long to wait for CHANNEL_OPENED response.
    /// Default: 10 seconds
    pub channel_timeout_secs: u64,

    /// Event buffer capacity.
    /// Number of events to buffer before backpressure.
    /// Default: 1024
    pub event_buffer_capacity: usize,

    /// Enable automatic quote token refresh.
    /// When enabled, the streamer proactively reconnects before the
    /// 24-hour token expiration to obtain a fresh token.
    /// Default: true
    pub auto_refresh_quote_token: bool,

    /// Quote token refresh interval in seconds.
    /// The streamer will reconnect after this duration to get a fresh token.
    /// Should be less than 24 hours (86400 seconds).
    /// Default: 82800 seconds (23 hours)
    pub quote_token_refresh_interval_secs: u64,
}

impl Default for DxLinkConfig {
    fn default() -> Self {
        Self {
            aggregation_period: 0.1,
            keepalive_interval_secs: 30,
            auth_timeout_secs: 10,
            channel_timeout_secs: 10,
            event_buffer_capacity: 1024,
            auto_refresh_quote_token: true,
            quote_token_refresh_interval_secs: 82800, // 23 hours
        }
    }
}

impl DxLinkConfig {
    /// Create a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the aggregation period in seconds.
    ///
    /// Lower values (e.g., 0.0) provide more real-time data but increase
    /// bandwidth usage. Higher values (e.g., 1.0) reduce bandwidth but
    /// add latency.
    pub fn with_aggregation_period(mut self, period: f64) -> Self {
        self.aggregation_period = period;
        self
    }

    /// Set the keepalive interval in seconds.
    ///
    /// Must be less than 60 seconds to prevent the server from closing
    /// the connection.
    pub fn with_keepalive_interval_secs(mut self, secs: u64) -> Self {
        self.keepalive_interval_secs = secs.min(59); // Must be < 60
        self
    }

    /// Set the authentication timeout in seconds.
    pub fn with_auth_timeout_secs(mut self, secs: u64) -> Self {
        self.auth_timeout_secs = secs;
        self
    }

    /// Set the channel open timeout in seconds.
    pub fn with_channel_timeout_secs(mut self, secs: u64) -> Self {
        self.channel_timeout_secs = secs;
        self
    }

    /// Set the event buffer capacity.
    pub fn with_event_buffer_capacity(mut self, capacity: usize) -> Self {
        self.event_buffer_capacity = capacity;
        self
    }

    /// Enable or disable automatic quote token refresh.
    ///
    /// When enabled, the streamer proactively reconnects before the 24-hour
    /// token expiration to obtain a fresh token.
    pub fn with_auto_refresh_quote_token(mut self, enabled: bool) -> Self {
        self.auto_refresh_quote_token = enabled;
        self
    }

    /// Set the quote token refresh interval in seconds.
    ///
    /// The streamer will reconnect after this duration to get a fresh token.
    /// Should be less than 24 hours (86400 seconds). Values >= 86400 will be
    /// capped at 82800 (23 hours).
    pub fn with_quote_token_refresh_interval_secs(mut self, secs: u64) -> Self {
        self.quote_token_refresh_interval_secs = secs.min(82800);
        self
    }

    /// Set the quote token refresh interval in hours.
    ///
    /// Convenience method that converts hours to seconds. Should be less than
    /// 24 hours. Values >= 24 will be capped at 23 hours.
    pub fn with_quote_token_refresh_hours(mut self, hours: u64) -> Self {
        let secs = (hours * 3600).min(82800);
        self.quote_token_refresh_interval_secs = secs;
        self
    }

    /// Create a low-latency configuration.
    ///
    /// Optimized for real-time trading with minimal aggregation.
    pub fn low_latency() -> Self {
        Self {
            aggregation_period: 0.0,
            keepalive_interval_secs: 15,
            auth_timeout_secs: 5,
            channel_timeout_secs: 5,
            event_buffer_capacity: 4096,
            auto_refresh_quote_token: true,
            quote_token_refresh_interval_secs: 82800,
        }
    }

    /// Create a bandwidth-optimized configuration.
    ///
    /// Reduces network usage at the cost of some latency.
    pub fn bandwidth_optimized() -> Self {
        Self {
            aggregation_period: 1.0,
            keepalive_interval_secs: 30,
            auth_timeout_secs: 15,
            channel_timeout_secs: 15,
            event_buffer_capacity: 512,
            auto_refresh_quote_token: true,
            quote_token_refresh_interval_secs: 82800,
        }
    }
}

/// Trait for DXLink event types.
pub trait DxEventTrait: Sized + for<'de> Deserialize<'de> {
    /// Get the event type.
    fn event_type() -> EventType;
}

/// Union of all DXLink event types.
///
/// Provides type-safe access to market data events with convenient helper methods
/// for filtering and extraction.
///
/// # Example
/// ```ignore
/// while let Some(event) = streamer.next_event().await? {
///     // Check event type
///     if event.is_quote() {
///         if let Some(quote) = event.as_quote() {
///             println!("Bid: {:?}, Ask: {:?}", quote.bid_price, quote.ask_price);
///         }
///     }
///
///     // Get the symbol for any event
///     println!("Symbol: {}", event.symbol());
///
///     // Get the event type
///     println!("Type: {:?}", event.event_type());
/// }
/// ```
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
    /// Time and sales event
    TimeAndSale(TimeAndSale),
    /// Extended trading hours trade event
    TradeETH(TradeETH),
    /// Underlying security event
    Underlying(Underlying),
}

impl DxEvent {
    /// Get the event type of this event.
    pub fn event_type(&self) -> EventType {
        match self {
            DxEvent::Quote(_) => EventType::Quote,
            DxEvent::Trade(_) => EventType::Trade,
            DxEvent::Greeks(_) => EventType::Greeks,
            DxEvent::Summary(_) => EventType::Summary,
            DxEvent::Profile(_) => EventType::Profile,
            DxEvent::Candle(_) => EventType::Candle,
            DxEvent::TheoPrice(_) => EventType::TheoPrice,
            DxEvent::TimeAndSale(_) => EventType::TimeAndSale,
            DxEvent::TradeETH(_) => EventType::TradeETH,
            DxEvent::Underlying(_) => EventType::Underlying,
        }
    }

    /// Get the symbol associated with this event.
    pub fn symbol(&self) -> &str {
        match self {
            DxEvent::Quote(e) => &e.event_symbol,
            DxEvent::Trade(e) => &e.event_symbol,
            DxEvent::Greeks(e) => &e.event_symbol,
            DxEvent::Summary(e) => &e.event_symbol,
            DxEvent::Profile(e) => &e.event_symbol,
            DxEvent::Candle(e) => &e.event_symbol,
            DxEvent::TheoPrice(e) => &e.event_symbol,
            DxEvent::TimeAndSale(e) => &e.event_symbol,
            DxEvent::TradeETH(e) => &e.event_symbol,
            DxEvent::Underlying(e) => &e.event_symbol,
        }
    }

    /// Returns true if this is a Quote event.
    pub fn is_quote(&self) -> bool {
        matches!(self, DxEvent::Quote(_))
    }

    /// Returns true if this is a Trade event.
    pub fn is_trade(&self) -> bool {
        matches!(self, DxEvent::Trade(_))
    }

    /// Returns true if this is a Greeks event.
    pub fn is_greeks(&self) -> bool {
        matches!(self, DxEvent::Greeks(_))
    }

    /// Returns true if this is a Summary event.
    pub fn is_summary(&self) -> bool {
        matches!(self, DxEvent::Summary(_))
    }

    /// Returns true if this is a Profile event.
    pub fn is_profile(&self) -> bool {
        matches!(self, DxEvent::Profile(_))
    }

    /// Returns true if this is a Candle event.
    pub fn is_candle(&self) -> bool {
        matches!(self, DxEvent::Candle(_))
    }

    /// Returns true if this is a TheoPrice event.
    pub fn is_theo_price(&self) -> bool {
        matches!(self, DxEvent::TheoPrice(_))
    }

    /// Returns true if this is a TimeAndSale event.
    pub fn is_time_and_sale(&self) -> bool {
        matches!(self, DxEvent::TimeAndSale(_))
    }

    /// Returns true if this is a TradeETH event.
    pub fn is_trade_eth(&self) -> bool {
        matches!(self, DxEvent::TradeETH(_))
    }

    /// Returns true if this is an Underlying event.
    pub fn is_underlying(&self) -> bool {
        matches!(self, DxEvent::Underlying(_))
    }

    /// Try to get a reference to the Quote, if this is a Quote event.
    pub fn as_quote(&self) -> Option<&Quote> {
        match self {
            DxEvent::Quote(q) => Some(q),
            _ => None,
        }
    }

    /// Try to get a reference to the Trade, if this is a Trade event.
    pub fn as_trade(&self) -> Option<&Trade> {
        match self {
            DxEvent::Trade(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get a reference to the Greeks, if this is a Greeks event.
    pub fn as_greeks(&self) -> Option<&Greeks> {
        match self {
            DxEvent::Greeks(g) => Some(g),
            _ => None,
        }
    }

    /// Try to get a reference to the Summary, if this is a Summary event.
    pub fn as_summary(&self) -> Option<&Summary> {
        match self {
            DxEvent::Summary(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get a reference to the Profile, if this is a Profile event.
    pub fn as_profile(&self) -> Option<&Profile> {
        match self {
            DxEvent::Profile(p) => Some(p),
            _ => None,
        }
    }

    /// Try to get a reference to the Candle, if this is a Candle event.
    pub fn as_candle(&self) -> Option<&Candle> {
        match self {
            DxEvent::Candle(c) => Some(c),
            _ => None,
        }
    }

    /// Try to get a reference to the TheoPrice, if this is a TheoPrice event.
    pub fn as_theo_price(&self) -> Option<&TheoPrice> {
        match self {
            DxEvent::TheoPrice(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get a reference to the TimeAndSale, if this is a TimeAndSale event.
    pub fn as_time_and_sale(&self) -> Option<&TimeAndSale> {
        match self {
            DxEvent::TimeAndSale(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get a reference to the TradeETH, if this is a TradeETH event.
    pub fn as_trade_eth(&self) -> Option<&TradeETH> {
        match self {
            DxEvent::TradeETH(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get a reference to the Underlying, if this is an Underlying event.
    pub fn as_underlying(&self) -> Option<&Underlying> {
        match self {
            DxEvent::Underlying(u) => Some(u),
            _ => None,
        }
    }
}

impl DxLinkStreamer {
    /// Connect to the DXLink streaming service with default configuration.
    pub(crate) async fn connect(client: Arc<ClientInner>) -> Result<Self> {
        Self::connect_with_config(client, DxLinkConfig::default()).await
    }

    /// Connect to the DXLink streaming service with custom configuration.
    pub(crate) async fn connect_with_config(
        client: Arc<ClientInner>,
        config: DxLinkConfig,
    ) -> Result<Self> {
        // Get the API quote token
        let token = Self::get_quote_token(&client).await?;

        // Connect to WebSocket
        let url = client.session.environment().await.dxlink_url();
        let (ws_stream, _) = connect_async(url).await?;

        // Perform authentication handshake (no channels opened yet)
        let (write, read) = Self::perform_auth_handshake(ws_stream, &token, &config).await?;

        let write = Arc::new(RwLock::new(write));
        let subscriptions = Arc::new(RwLock::new(HashMap::new()));
        let channel_states = Arc::new(RwLock::new(HashMap::new()));
        let connected = Arc::new(AtomicBool::new(true));
        let quote_token_obtained_at = Arc::new(RwLock::new(Utc::now()));
        let needs_token_refresh = Arc::new(AtomicBool::new(false));

        // Create event channel with configured capacity
        let (event_tx, event_rx) = mpsc::channel(config.event_buffer_capacity);

        // Start message processing task
        let write_clone = write.clone();
        let subs_clone = subscriptions.clone();
        let connected_clone = connected.clone();
        let channel_states_clone = channel_states.clone();
        tokio::spawn(async move {
            Self::process_messages(read, event_tx, write_clone, subs_clone, connected_clone, channel_states_clone).await;
        });

        // Start keepalive task with configured interval
        let keepalive_write = write.clone();
        let keepalive_connected = connected.clone();
        let keepalive_interval = config.keepalive_interval_secs;
        tokio::spawn(async move {
            Self::keepalive_task_with_interval(keepalive_write, keepalive_connected, keepalive_interval).await;
        });

        // Start quote token refresh task if enabled
        if config.auto_refresh_quote_token {
            let refresh_connected = connected.clone();
            let refresh_flag = needs_token_refresh.clone();
            let refresh_timestamp = quote_token_obtained_at.clone();
            let refresh_interval = config.quote_token_refresh_interval_secs;
            tokio::spawn(async move {
                Self::quote_token_refresh_task(
                    refresh_connected,
                    refresh_flag,
                    refresh_timestamp,
                    refresh_interval,
                )
                .await;
            });
        }

        Ok(Self {
            write,
            event_rx,
            subscriptions,
            channel_states,
            connected,
            client,
            reconnect_config: ReconnectConfig::default(),
            config,
            quote_token_obtained_at,
            needs_token_refresh,
        })
    }

    /// Get the current DXLink configuration.
    pub fn config(&self) -> &DxLinkConfig {
        &self.config
    }

    /// Subscribe to events for the given symbols.
    ///
    /// Each symbol is sent as a separate entry in the subscription message,
    /// per DXLink protocol requirements. Channels are opened automatically
    /// on first subscription for each event type.
    pub async fn subscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();
        let channel_id = event_type.channel_id();

        // Ensure the channel for this event type is open
        self.ensure_channel_open(event_type).await?;

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
            "channel": channel_id,
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

    /// Ensure the channel for an event type is open.
    ///
    /// Opens the channel if not already open, sending CHANNEL_REQUEST and FEED_SETUP.
    async fn ensure_channel_open(&self, event_type: EventType) -> Result<()> {
        let channel_id = event_type.channel_id();

        // Check if channel is already open
        {
            let states = self.channel_states.read().await;
            if let Some(ChannelState::Opened) = states.get(&channel_id) {
                return Ok(());
            }
        }

        // Mark channel as opening
        {
            let mut states = self.channel_states.write().await;
            states.insert(channel_id, ChannelState::Opening);
        }

        // Send CHANNEL_REQUEST
        let channel_request = serde_json::json!({
            "type": "CHANNEL_REQUEST",
            "channel": channel_id,
            "service": "FEED",
            "parameters": {
                "contract": "AUTO"
            }
        });
        self.send_message(&channel_request).await?;

        // Wait for CHANNEL_OPENED (handled by process_messages)
        // Poll the channel state until opened or timeout
        let timeout = Duration::from_secs(self.config.channel_timeout_secs);
        let start = std::time::Instant::now();

        loop {
            {
                let states = self.channel_states.read().await;
                if let Some(ChannelState::Opened) = states.get(&channel_id) {
                    break;
                }
            }

            if start.elapsed() > timeout {
                return Err(Error::Timeout(format!(
                    "Waiting for CHANNEL_OPENED on channel {}",
                    channel_id
                )));
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Send FEED_SETUP for this channel using configured aggregation period
        let feed_setup = serde_json::json!({
            "type": "FEED_SETUP",
            "channel": channel_id,
            "acceptAggregationPeriod": self.config.aggregation_period,
            "acceptDataFormat": "COMPACT",
            "acceptEventFields": compact::get_accept_event_fields()
        });
        self.send_message(&feed_setup).await?;

        Ok(())
    }

    /// Subscribe to candle events for the given symbols with specified period.
    ///
    /// Candle subscriptions require special symbol syntax. This helper method
    /// constructs the proper candle symbol format: `SYMBOL{=PERIOD}`.
    ///
    /// # Arguments
    /// * `symbols` - Base symbols to subscribe to (e.g., "AAPL", "SPY")
    /// * `period` - Candle period specification
    ///
    /// # Example
    /// ```ignore
    /// // Subscribe to daily candles for AAPL and SPY
    /// streamer.subscribe_candles(&["AAPL", "SPY"], CandlePeriod::Day).await?;
    ///
    /// // Subscribe to hourly candles
    /// streamer.subscribe_candles(&["AAPL"], CandlePeriod::Hour).await?;
    /// ```
    pub async fn subscribe_candles(
        &mut self,
        symbols: &[&str],
        period: CandlePeriod,
    ) -> Result<()> {
        let candle_symbols: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}{{{}}}", s, period.as_str()))
            .collect();

        let symbol_refs: Vec<&str> = candle_symbols.iter().map(|s| s.as_str()).collect();
        self.subscribe::<Candle>(&symbol_refs).await
    }

    /// Unsubscribe from candle events for the given symbols with specified period.
    ///
    /// # Arguments
    /// * `symbols` - Base symbols to unsubscribe from (e.g., "AAPL", "SPY")
    /// * `period` - Candle period specification (must match the subscription)
    pub async fn unsubscribe_candles(
        &mut self,
        symbols: &[&str],
        period: CandlePeriod,
    ) -> Result<()> {
        let candle_symbols: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}{{{}}}", s, period.as_str()))
            .collect();

        let symbol_refs: Vec<&str> = candle_symbols.iter().map(|s| s.as_str()).collect();
        self.unsubscribe::<Candle>(&symbol_refs).await
    }

    /// Unsubscribe from events for the given symbols.
    ///
    /// Each symbol is sent as a separate entry in the unsubscribe message,
    /// per DXLink protocol requirements.
    pub async fn unsubscribe<E: DxEventTrait>(&mut self, symbols: &[&str]) -> Result<()> {
        let event_type = E::event_type();
        let channel_id = event_type.channel_id();

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
            "channel": channel_id,
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

    /// Get the state of a specific channel.
    pub async fn channel_state(&self, channel_id: i32) -> ChannelState {
        let states = self.channel_states.read().await;
        states.get(&channel_id).copied().unwrap_or(ChannelState::Closed)
    }

    /// Get all open channels.
    pub async fn open_channels(&self) -> Vec<i32> {
        let states = self.channel_states.read().await;
        states
            .iter()
            .filter(|(_, state)| **state == ChannelState::Opened)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get the next event with automatic quote token refresh.
    ///
    /// If automatic quote token refresh is enabled and the refresh interval
    /// has elapsed, this method will automatically reconnect to obtain a
    /// fresh token before returning the next event.
    ///
    /// # Example
    /// ```ignore
    /// while let Some(event) = streamer.next().await {
    ///     match event? {
    ///         DxEvent::Quote(quote) => println!("{:?}", quote),
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub async fn next(&mut self) -> Option<Result<DxEvent>> {
        // Check if quote token refresh is needed
        if self.config.auto_refresh_quote_token
            && self.needs_token_refresh.load(Ordering::SeqCst)
        {
            tracing::info!("Performing automatic quote token refresh reconnection");
            match self.reconnect().await {
                Ok(()) => {
                    tracing::info!("Automatic quote token refresh completed successfully");
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "Automatic quote token refresh failed, will retry on next check"
                    );
                    // Don't clear the flag so it will retry
                    // Return the error to the caller
                    return Some(Err(e));
                }
            }
        }

        self.event_rx.recv().await
    }

    /// Check if the WebSocket connection is currently active.
    ///
    /// Returns `true` if the connection is established and healthy.
    /// Note that this is an atomic check and the state may change immediately after.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Get the timestamp when the current quote token was obtained.
    ///
    /// This can be used to monitor token age and determine when the next
    /// automatic refresh will occur.
    pub async fn quote_token_obtained_at(&self) -> DateTime<Utc> {
        *self.quote_token_obtained_at.read().await
    }

    /// Check if quote token refresh is needed based on the configured interval.
    ///
    /// Returns `true` if the token age exceeds the refresh interval.
    /// When automatic refresh is enabled, the streamer will automatically
    /// reconnect when this condition is detected.
    pub async fn needs_quote_token_refresh(&self) -> bool {
        if !self.config.auto_refresh_quote_token {
            return false;
        }

        let token_age = {
            let timestamp = self.quote_token_obtained_at.read().await;
            Utc::now().signed_duration_since(*timestamp)
        };

        token_age.num_seconds() >= self.config.quote_token_refresh_interval_secs as i64
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
    /// 4. Re-open channels and re-subscribe to all previously subscribed symbols
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

        // Reset channel states (all channels need to be re-opened)
        {
            let mut states = self.channel_states.write().await;
            states.clear();
        }

        // Get a fresh quote token
        let token = Self::get_quote_token(&self.client).await?;

        // Update quote token timestamp
        {
            let mut timestamp = self.quote_token_obtained_at.write().await;
            *timestamp = Utc::now();
        }

        // Clear refresh flag
        self.needs_token_refresh.store(false, Ordering::SeqCst);

        // Connect to WebSocket
        let url = self.client.session.environment().await.dxlink_url();
        let (ws_stream, _) = connect_async(url).await?;

        // Perform authentication handshake (no channels opened yet)
        let (write, read) = Self::perform_auth_handshake(ws_stream, &token, &self.config).await?;

        // Update write handle
        *self.write.write().await = write;

        // Create new event channel with configured capacity
        let (event_tx, event_rx) = mpsc::channel(self.config.event_buffer_capacity);
        self.event_rx = event_rx;

        // Mark as connected
        self.connected.store(true, Ordering::SeqCst);

        // Start message processing task
        let write_clone = self.write.clone();
        let subs_clone = self.subscriptions.clone();
        let connected_clone = self.connected.clone();
        let channel_states_clone = self.channel_states.clone();
        tokio::spawn(async move {
            Self::process_messages(read, event_tx, write_clone, subs_clone, connected_clone, channel_states_clone).await;
        });

        // Start new keepalive task with configured interval
        let keepalive_write = self.write.clone();
        let keepalive_connected = self.connected.clone();
        let keepalive_interval = self.config.keepalive_interval_secs;
        tokio::spawn(async move {
            Self::keepalive_task_with_interval(keepalive_write, keepalive_connected, keepalive_interval).await;
        });

        // Start new quote token refresh task if enabled
        if self.config.auto_refresh_quote_token {
            let refresh_connected = self.connected.clone();
            let refresh_flag = self.needs_token_refresh.clone();
            let refresh_timestamp = self.quote_token_obtained_at.clone();
            let refresh_interval = self.config.quote_token_refresh_interval_secs;
            tokio::spawn(async move {
                Self::quote_token_refresh_task(
                    refresh_connected,
                    refresh_flag,
                    refresh_timestamp,
                    refresh_interval,
                )
                .await;
            });
        }

        // Re-subscribe to all symbols, grouped by event type
        // This will automatically re-open channels via ensure_channel_open
        let mut by_type: HashMap<EventType, Vec<String>> = HashMap::new();
        for (symbol, event_type) in subscriptions {
            by_type.entry(event_type).or_default().push(symbol);
        }

        for (event_type, symbols) in by_type {
            // Ensure channel is open for this event type
            self.ensure_channel_open(event_type).await?;

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

            let channel_id = event_type.channel_id();
            let msg = serde_json::json!({
                "type": "FEED_SUBSCRIPTION",
                "channel": channel_id,
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

    /// DXLink protocol version.
    const DXLINK_VERSION: &'static str = "0.1-DXF-JS/0.3.0";

    /// Perform the authentication handshake with the DXLink server.
    ///
    /// This implements the authentication portion of the DXLink protocol:
    /// 1. SETUP → wait for SETUP response
    /// 2. AUTH → wait for AUTH_STATE: AUTHORIZED
    ///
    /// Channels are opened lazily on first subscription to each event type.
    /// Returns the split WebSocket stream after successful authentication.
    async fn perform_auth_handshake(
        mut ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        token: &str,
        config: &DxLinkConfig,
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
        let auth_timeout = Duration::from_secs(config.auth_timeout_secs);

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
                                            // Authentication successful, split and return
                                            return Ok(ws_stream.split());
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
    }

    async fn send_message(&self, msg: &serde_json::Value) -> Result<()> {
        let mut write = self.write.write().await;
        write.send(Message::Text(msg.to_string())).await?;
        Ok(())
    }

    /// Background task that sends KEEPALIVE messages at the configured interval.
    ///
    /// Per DXLink protocol, the client must proactively send KEEPALIVE
    /// to prevent the 60-second timeout from closing the connection.
    async fn keepalive_task_with_interval(
        write: Arc<RwLock<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >>>,
        connected: Arc<AtomicBool>,
        interval_secs: u64,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

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

    /// Background task that monitors quote token age and signals when refresh is needed.
    ///
    /// Quote tokens expire after 24 hours. This task checks the token age periodically
    /// and sets the `needs_token_refresh` flag when the configured refresh interval
    /// has elapsed, triggering automatic reconnection to obtain a fresh token.
    async fn quote_token_refresh_task(
        connected: Arc<AtomicBool>,
        needs_refresh: Arc<AtomicBool>,
        token_timestamp: Arc<RwLock<DateTime<Utc>>>,
        refresh_interval_secs: u64,
    ) {
        // Check every hour if refresh is needed
        let check_interval = Duration::from_secs(3600); // 1 hour
        let mut interval = tokio::time::interval(check_interval);

        loop {
            interval.tick().await;

            // Stop if disconnected
            if !connected.load(Ordering::SeqCst) {
                return;
            }

            // Check if token age exceeds refresh interval
            let token_age = {
                let timestamp = token_timestamp.read().await;
                Utc::now().signed_duration_since(*timestamp)
            };

            if token_age.num_seconds() >= refresh_interval_secs as i64 {
                tracing::info!(
                    token_age_hours = token_age.num_hours(),
                    refresh_interval_hours = refresh_interval_secs / 3600,
                    "Quote token refresh interval reached, triggering automatic reconnection"
                );
                needs_refresh.store(true, Ordering::SeqCst);
                // Sleep briefly to avoid busy loop if reconnection fails
                tokio::time::sleep(Duration::from_secs(60)).await;
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
        channel_states: Arc<RwLock<HashMap<i32, ChannelState>>>,
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
                                "CHANNEL_OPENED" => {
                                    // Update channel state to Opened
                                    if let Some(channel_id) = json.get("channel").and_then(|c| c.as_i64()) {
                                        let mut states = channel_states.write().await;
                                        states.insert(channel_id as i32, ChannelState::Opened);
                                    }
                                }
                                "CHANNEL_CLOSED" => {
                                    // Update channel state to Closed
                                    if let Some(channel_id) = json.get("channel").and_then(|c| c.as_i64()) {
                                        let mut states = channel_states.write().await;
                                        states.insert(channel_id as i32, ChannelState::Closed);
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
                Ok(Message::Close(close_frame)) => {
                    connected.store(false, Ordering::SeqCst);

                    // Check for specific close codes
                    if let Some(frame) = close_frame {
                        let code = frame.code.into();
                        let reason = frame.reason.to_string();

                        // Handle 1009 (Message Too Big) specially
                        if code == 1009 {
                            let _ = event_tx.send(Err(Error::MessageTooLarge(format!(
                                "WebSocket closed with code 1009: {}. \
                                Reduce subscription batch sizes.",
                                reason
                            )))).await;
                            return;
                        }

                        // Handle other close codes
                        let _ = event_tx.send(Err(Error::from_ws_close_code(code, &reason))).await;
                    } else {
                        let _ = event_tx.send(Err(Error::StreamDisconnected)).await;
                    }
                    return;
                }
                Err(e) => {
                    connected.store(false, Ordering::SeqCst);
                    // Convert the error, which may be a 1009 error
                    let _ = event_tx.send(Err(e.into())).await;
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

    #[test]
    fn test_candle_period_as_str() {
        assert_eq!(CandlePeriod::Tick.as_str(), "=t");
        assert_eq!(CandlePeriod::Second.as_str(), "=s");
        assert_eq!(CandlePeriod::Minute.as_str(), "=m");
        assert_eq!(CandlePeriod::Minutes(5).as_str(), "=5m");
        assert_eq!(CandlePeriod::Minutes(15).as_str(), "=15m");
        assert_eq!(CandlePeriod::Hour.as_str(), "=h");
        assert_eq!(CandlePeriod::Hours(4).as_str(), "=4h");
        assert_eq!(CandlePeriod::Day.as_str(), "=d");
        assert_eq!(CandlePeriod::Week.as_str(), "=w");
        assert_eq!(CandlePeriod::Month.as_str(), "=mo");
        assert_eq!(CandlePeriod::Year.as_str(), "=y");
    }

    #[test]
    fn test_candle_symbol_format() {
        // Verify the expected candle symbol format
        let symbol = "AAPL";
        let period = CandlePeriod::Day;
        let candle_symbol = format!("{}{{{}}}", symbol, period.as_str());
        assert_eq!(candle_symbol, "AAPL{=d}");

        let period = CandlePeriod::Minutes(5);
        let candle_symbol = format!("{}{{{}}}", symbol, period.as_str());
        assert_eq!(candle_symbol, "AAPL{=5m}");
    }

    #[test]
    fn test_event_type_channel_ids() {
        // Verify channel IDs are unique for each event type
        let mut channel_ids: std::collections::HashSet<i32> = std::collections::HashSet::new();
        for event_type in EventType::all() {
            let channel_id = event_type.channel_id();
            assert!(
                channel_ids.insert(channel_id),
                "Duplicate channel ID {} for {:?}",
                channel_id,
                event_type
            );
        }
    }

    #[test]
    fn test_channel_state_default() {
        // Verify default channel state behavior
        assert_eq!(ChannelState::Closed, ChannelState::Closed);
        assert_ne!(ChannelState::Closed, ChannelState::Opened);
    }

    #[test]
    fn test_dxlink_config_default() {
        let config = DxLinkConfig::default();
        assert_eq!(config.aggregation_period, 0.1);
        assert_eq!(config.keepalive_interval_secs, 30);
        assert_eq!(config.auth_timeout_secs, 10);
        assert_eq!(config.channel_timeout_secs, 10);
        assert_eq!(config.event_buffer_capacity, 1024);
        assert_eq!(config.auto_refresh_quote_token, true);
        assert_eq!(config.quote_token_refresh_interval_secs, 82800); // 23 hours
    }

    #[test]
    fn test_dxlink_config_low_latency() {
        let config = DxLinkConfig::low_latency();
        assert_eq!(config.aggregation_period, 0.0);
        assert_eq!(config.keepalive_interval_secs, 15);
        assert_eq!(config.event_buffer_capacity, 4096);
    }

    #[test]
    fn test_dxlink_config_bandwidth_optimized() {
        let config = DxLinkConfig::bandwidth_optimized();
        assert_eq!(config.aggregation_period, 1.0);
        assert_eq!(config.event_buffer_capacity, 512);
    }

    #[test]
    fn test_dxlink_config_builder() {
        let config = DxLinkConfig::new()
            .with_aggregation_period(0.5)
            .with_keepalive_interval_secs(20)
            .with_event_buffer_capacity(2048);

        assert_eq!(config.aggregation_period, 0.5);
        assert_eq!(config.keepalive_interval_secs, 20);
        assert_eq!(config.event_buffer_capacity, 2048);
    }

    #[test]
    fn test_dxlink_config_keepalive_max() {
        // Keepalive interval should be capped at 59 seconds
        let config = DxLinkConfig::new().with_keepalive_interval_secs(120);
        assert_eq!(config.keepalive_interval_secs, 59);
    }

    #[test]
    fn test_dxlink_config_auto_refresh_enabled() {
        let config = DxLinkConfig::new().with_auto_refresh_quote_token(true);
        assert_eq!(config.auto_refresh_quote_token, true);
    }

    #[test]
    fn test_dxlink_config_auto_refresh_disabled() {
        let config = DxLinkConfig::new().with_auto_refresh_quote_token(false);
        assert_eq!(config.auto_refresh_quote_token, false);
    }

    #[test]
    fn test_dxlink_config_refresh_interval_secs() {
        let config = DxLinkConfig::new().with_quote_token_refresh_interval_secs(3600);
        assert_eq!(config.quote_token_refresh_interval_secs, 3600);
    }

    #[test]
    fn test_dxlink_config_refresh_interval_secs_max() {
        // Refresh interval should be capped at 82800 seconds (23 hours)
        let config = DxLinkConfig::new().with_quote_token_refresh_interval_secs(100000);
        assert_eq!(config.quote_token_refresh_interval_secs, 82800);
    }

    #[test]
    fn test_dxlink_config_refresh_interval_hours() {
        let config = DxLinkConfig::new().with_quote_token_refresh_hours(20);
        assert_eq!(config.quote_token_refresh_interval_secs, 72000); // 20 * 3600
    }

    #[test]
    fn test_dxlink_config_refresh_interval_hours_max() {
        // Refresh interval should be capped at 23 hours when using hours
        let config = DxLinkConfig::new().with_quote_token_refresh_hours(25);
        assert_eq!(config.quote_token_refresh_interval_secs, 82800); // 23 hours max
    }

    #[test]
    fn test_dxlink_config_builder_with_refresh_options() {
        let config = DxLinkConfig::new()
            .with_aggregation_period(0.5)
            .with_auto_refresh_quote_token(false)
            .with_quote_token_refresh_hours(22);

        assert_eq!(config.aggregation_period, 0.5);
        assert_eq!(config.auto_refresh_quote_token, false);
        assert_eq!(config.quote_token_refresh_interval_secs, 79200); // 22 hours
    }

    #[test]
    fn test_dxlink_config_low_latency_has_auto_refresh() {
        let config = DxLinkConfig::low_latency();
        assert_eq!(config.auto_refresh_quote_token, true);
        assert_eq!(config.quote_token_refresh_interval_secs, 82800);
    }

    #[test]
    fn test_dxlink_config_bandwidth_optimized_has_auto_refresh() {
        let config = DxLinkConfig::bandwidth_optimized();
        assert_eq!(config.auto_refresh_quote_token, true);
        assert_eq!(config.quote_token_refresh_interval_secs, 82800);
    }

    #[test]
    fn test_dx_event_type_helpers() {
        let quote = DxEvent::Quote(Quote {
            event_symbol: "AAPL".to_string(),
            event_time: None,
            sequence: None,
            time_nano_part: None,
            bid_time: None,
            bid_exchange_code: None,
            bid_price: None,
            bid_size: None,
            ask_time: None,
            ask_exchange_code: None,
            ask_price: None,
            ask_size: None,
        });

        assert!(quote.is_quote());
        assert!(!quote.is_trade());
        assert_eq!(quote.event_type(), EventType::Quote);
        assert_eq!(quote.symbol(), "AAPL");
        assert!(quote.as_quote().is_some());
        assert!(quote.as_trade().is_none());
    }

    #[test]
    fn test_dx_event_symbol() {
        let trade = DxEvent::Trade(Trade {
            event_symbol: "SPY".to_string(),
            event_time: None,
            event_flags: None,
            index: None,
            time: None,
            time_nano_part: None,
            sequence: None,
            exchange_code: None,
            price: None,
            change: None,
            size: None,
            day_volume: None,
            day_turnover: None,
            tick_direction: None,
            extended_trading_hours: None,
        });

        assert_eq!(trade.symbol(), "SPY");
        assert!(trade.is_trade());
        assert!(trade.as_trade().is_some());
    }
}
