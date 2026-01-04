# DXLink Market Data Streaming - Gap Analysis

## Executive Summary

This document provides a comprehensive gap analysis comparing the Rust `tastytrade-rs` library's DXLink market data streaming implementation against:
1. TastyTrade's official streaming documentation
2. The reference Python `tastytrade` library implementation
3. DXLink protocol specifications

**Critical Finding**: The current Rust implementation has **15 identified gaps**, including **5 critical** issues that prevent reliable operation in production environments.

---

## Gap Analysis

### GAP-1: COMPACT Data Format Parsing Not Implemented [CRITICAL]

**Severity**: Critical
**Component**: `src/streaming/dxlink/mod.rs` - `parse_feed_data()`

**Current State**:
The Rust implementation expects FEED_DATA in FULL JSON format with named fields:
```rust
if let Some(event_type) = item.get("eventType").and_then(|t| t.as_str()) {
    // Parses item as JSON object with named fields
}
```

**Required State**:
DXLink requires COMPACT format where data comes as **positional arrays**, not named objects:
```json
{"type":"FEED_DATA","channel":3,"data":["Trade",["Trade","SPY",559.36,1.3743299E7,100.0]]}
```

The data array structure is:
- `data[0]`: Event type string ("Trade", "Quote", etc.)
- `data[1]`: Array of events, each as a positional array matching `acceptEventFields` order

**Python Reference**:
```python
def _map_message(self, message) -> None:
    msg_type = message.get('type')
    if msg_type == 'FEED_DATA':
        # Data comes as [EventType, [event1_array, event2_array, ...]]
        event_type = message['data'][0]
        events = message['data'][1]
        cls = EVENT_TYPE_MAP[event_type]
        for event_data in events:
            event = cls.from_stream(event_data)  # Positional deserialization
```

**Remediation**:
1. Implement positional array parsing based on `acceptEventFields` order
2. Create a field-to-position mapping per event type
3. Add `from_compact()` method to each event struct

---

### GAP-2: Missing FEED_SETUP Message [CRITICAL]

**Severity**: Critical
**Component**: `src/streaming/dxlink/mod.rs` - `authenticate()`

**Current State**:
The authentication flow sends SETUP → AUTH → CHANNEL_REQUEST but **skips FEED_SETUP**.

**Required State**:
Per DXLink protocol, FEED_SETUP is mandatory before subscriptions:
```json
{
  "type": "FEED_SETUP",
  "channel": 1,
  "acceptAggregationPeriod": 0.1,
  "acceptDataFormat": "COMPACT",
  "acceptEventFields": {
    "Quote": ["eventType", "eventSymbol", "bidPrice", "askPrice", "bidSize", "askSize"],
    "Trade": ["eventType", "eventSymbol", "price", "dayVolume", "size"],
    "Greeks": ["eventType", "eventSymbol", "volatility", "delta", "gamma", "theta", "rho", "vega"],
    "Summary": ["eventType", "eventSymbol", "openInterest", "dayOpenPrice", "dayHighPrice", "dayLowPrice", "prevDayClosePrice"],
    "Profile": ["eventType", "eventSymbol", "description", "shortSaleRestriction", "tradingStatus", "statusReason", "haltStartTime", "haltEndTime", "highLimitPrice", "lowLimitPrice", "high52WeekPrice", "low52WeekPrice"]
  }
}
```

**Impact**: Without FEED_SETUP, the server may reject subscriptions or send data in unexpected formats.

**Remediation**:
1. Add FEED_SETUP message after CHANNEL_OPENED is received
2. Make `acceptEventFields` configurable
3. Store field order mapping for COMPACT parsing

---

### GAP-3: No Proactive Keepalive Task [CRITICAL]

**Severity**: Critical
**Component**: `src/streaming/dxlink/mod.rs` - `process_messages()`

**Current State**:
The code only **responds** to incoming KEEPALIVE messages:
```rust
"KEEPALIVE" => {
    let response = serde_json::json!({"type": "KEEPALIVE", "channel": 0});
    let _ = w.send(Message::Text(response.to_string())).await;
}
```

**Required State**:
Per protocol, the client must **proactively send** KEEPALIVE every 30 seconds to prevent the 60-second timeout:
```
If DxLink doesn't receive a keepalive within the 60-second timeout, it will close the connection.
Sending a keepalive message every 30 seconds will keep the connection alive indefinitely.
```

**Python Reference**:
```python
async def _heartbeat(self) -> None:
    """Send keepalive messages every 30 seconds"""
    while True:
        await asyncio.sleep(30)
        await self._websocket.send(json.dumps({'type': 'KEEPALIVE', 'channel': 0}))
```

**Remediation**:
1. Add a background keepalive task that sends KEEPALIVE every 30 seconds
2. Start the task after successful authentication
3. Cancel the task on disconnect

---

### GAP-4: Missing AUTH_STATE Verification [HIGH]

**Severity**: High
**Component**: `src/streaming/dxlink/mod.rs` - `authenticate()`

**Current State**:
The code sends AUTH message but doesn't verify the AUTH_STATE response:
```rust
// Send AUTH message
let auth = serde_json::json!({"type": "AUTH", "channel": 0, "token": token});
w.send(Message::Text(auth.to_string())).await?;
// No verification of AUTH_STATE: AUTHORIZED
```

**Required State**:
Must wait for and verify:
```json
{"type":"AUTH_STATE","channel":0,"state":"AUTHORIZED","userId":"<redacted>"}
```

**Impact**: Silent authentication failures, subscriptions sent before authorization.

**Remediation**:
1. Wait for AUTH_STATE message after sending AUTH
2. Verify state is "AUTHORIZED"
3. Return authentication error if state is "UNAUTHORIZED"

---

### GAP-5: Missing CHANNEL_OPENED Verification [HIGH]

**Severity**: High
**Component**: `src/streaming/dxlink/mod.rs` - `authenticate()`

**Current State**:
CHANNEL_REQUEST is sent but CHANNEL_OPENED response is not verified.

**Required State**:
Must wait for:
```json
{"type":"CHANNEL_OPENED","channel":1,"service":"FEED","parameters":{"contract":"AUTO","subFormat":"LIST"}}
```

**Impact**: Subscriptions may be sent before channel is ready.

**Remediation**:
1. Wait for CHANNEL_OPENED before returning from channel setup
2. Track channel state per channel ID
3. Only allow subscriptions on opened channels

---

### GAP-6: Incorrect Subscription Message Format [HIGH]

**Severity**: High
**Component**: `src/streaming/dxlink/mod.rs` - `subscribe()`

**Current State**:
```rust
let msg = serde_json::json!({
    "type": "FEED_SUBSCRIPTION",
    "channel": self.channel_id,
    "add": [{
        "type": event_type.as_str(),
        "symbol": symbols,  // Array of symbols in single entry - WRONG
    }]
});
```

**Required State**:
Each symbol requires a separate entry in the add array:
```json
{
  "type": "FEED_SUBSCRIPTION",
  "channel": 1,
  "add": [
    {"type": "Quote", "symbol": "SPY"},
    {"type": "Quote", "symbol": "AAPL"},
    {"type": "Trade", "symbol": "SPY"}
  ]
}
```

**Remediation**:
```rust
let add: Vec<_> = symbols.iter().map(|s| {
    serde_json::json!({"type": event_type.as_str(), "symbol": s})
}).collect();
let msg = serde_json::json!({
    "type": "FEED_SUBSCRIPTION",
    "channel": self.channel_id,
    "add": add
});
```

---

### GAP-7: Single Channel Architecture [MEDIUM]

**Severity**: Medium
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
Uses a single channel (channel_id: 1) for all event types.

**Python Reference**:
Uses separate channels per event type:
```python
_channel_map = {
    Candle: 1, Greeks: 3, Profile: 5, Quote: 7,
    Summary: 9, TheoPrice: 11, TimeAndSale: 13, Trade: 15, Underlying: 17
}
```

**Impact**:
- Cannot configure different aggregation periods per event type
- Cannot selectively close channels
- Potential message routing issues

**Remediation**:
1. Implement channel-per-event-type architecture
2. Add channel state tracking
3. Support dynamic channel opening/closing

---

### GAP-8: No Reconnection Support [MEDIUM]

**Severity**: Medium
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No automatic reconnection capability. On disconnect:
```rust
Ok(Message::Close(_)) => {
    let _ = event_tx.send(Err(Error::StreamDisconnected)).await;
    return;  // Simply exits
}
```

**Python Reference**:
```python
async def listen(self, event_type):
    while True:
        try:
            async for websocket in connect(self.base_url):
                # Auto-reconnect loop with exponential backoff
                if self.reconnect_fn:
                    await self.reconnect_fn(self, *self.reconnect_args)
```

**Remediation**:
1. Add `DxLinkReconnectConfig` similar to account streamer
2. Implement reconnection loop with exponential backoff
3. Support `reconnect_fn` callback for re-subscription
4. Add `is_connected()` state tracking

---

### GAP-9: Missing Event Types [MEDIUM]

**Severity**: Medium
**Component**: `src/streaming/dxlink/events.rs`

**Current State**:
Missing event types that are supported by DXLink:
- `TimeAndSale` - Individual trade details with conditions
- `TradeETH` - Extended trading hours trades
- `Underlying` - Underlying security information

**Remediation**:
Add the missing event types:
```rust
pub struct TimeAndSale { /* fields */ }
pub struct TradeETH { /* fields */ }
pub struct Underlying { /* fields */ }
```

---

### GAP-10: No Connection State Tracking [MEDIUM]

**Severity**: Medium
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No way to check if the streamer is connected.

**Required State** (matching account streamer pattern):
```rust
impl DxLinkStreamer {
    pub fn is_connected(&self) -> bool;
    pub async fn reconnect(&mut self) -> Result<()>;
}
```

**Remediation**:
Add `connected: Arc<AtomicBool>` field and tracking methods.

---

### GAP-11: Missing Candle Subscription Helpers [MEDIUM]

**Severity**: Medium
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No helper for building candle symbols with proper syntax.

**Required Candle Symbol Format**:
```
AAPL{=5m}         - 5-minute candles
AAPL{=1h}         - 1-hour candles
AAPL{=1d}         - Daily candles
AAPL{=5m,tho=true} - Extended hours excluded
```

**Python Reference**:
```python
def _get_candle_symbol(symbol: str, interval: timedelta, extended: bool) -> str:
    period = _timedelta_to_period(interval)
    return f"{symbol}{{={period},tho={str(not extended).lower()}}}"
```

**Remediation**:
```rust
impl DxLinkStreamer {
    pub async fn subscribe_candles(
        &mut self,
        symbols: &[&str],
        interval: Duration,
        from_time: i64,
        extended_hours: bool,
    ) -> Result<()>;
}
```

---

### GAP-12: No Channel State Management [LOW]

**Severity**: Low
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No tracking of channel states (CHANNEL_CLOSED, CHANNEL_OPENED, etc.)

**Python Reference**:
```python
_subscription_state: dict[str, str] = field(default_factory=lambda: defaultdict(lambda: 'CHANNEL_CLOSED'))
```

**Remediation**:
Add channel state enum and tracking:
```rust
enum ChannelState { Closed, Opening, Opened, Closing }
channel_states: HashMap<i32, ChannelState>
```

---

### GAP-13: Missing Error Code 1009 Handling [LOW]

**Severity**: Low
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No specific handling for WebSocket close code 1009 (message too large).

**Python Reference**:
```python
if e.code == 1009:
    logger.error('Subscription message too large. Try subscribing with fewer symbols.')
```

**Remediation**:
Add specific error handling and user guidance for code 1009.

---

### GAP-14: No Configuration Options [LOW]

**Severity**: Low
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
No configurable options for:
- Aggregation period
- Event fields to subscribe
- Reconnection behavior

**Python Reference**:
```python
async def subscribe(self, event_type, symbols, refresh_interval=0.1):
    # Configurable refresh_interval per subscription
```

**Remediation**:
Add `DxLinkConfig` struct:
```rust
pub struct DxLinkConfig {
    pub aggregation_period: f64,
    pub reconnect_config: DxLinkReconnectConfig,
    pub event_fields: HashMap<EventType, Vec<String>>,
}
```

---

### GAP-15: No Typed Event Filtering [LOW]

**Severity**: Low
**Component**: `src/streaming/dxlink/mod.rs`

**Current State**:
`next()` returns all event types mixed together.

**Python Reference**:
```python
async def listen(self, event_type: Type[Event]) -> AsyncIterator[Event]:
    """Listen for events of a specific type"""
    async for event in self._queues[event_type]:
        yield event
```

**Remediation**:
Add typed event listening:
```rust
pub async fn listen<E: DxEventTrait>(&mut self) -> impl Stream<Item = Result<E>>;
```

---

## Implementation Priority Matrix

| Priority | Gap | Effort | Impact |
|----------|-----|--------|--------|
| P0 | GAP-1: COMPACT Parsing | High | Critical - Data cannot be parsed |
| P0 | GAP-2: FEED_SETUP | Medium | Critical - Subscriptions may fail |
| P0 | GAP-3: Keepalive Task | Low | Critical - Connection will drop |
| P1 | GAP-4: AUTH_STATE Verification | Low | High - Silent auth failures |
| P1 | GAP-5: CHANNEL_OPENED Verification | Low | High - Race conditions |
| P1 | GAP-6: Subscription Format | Low | High - Subscriptions fail |
| P2 | GAP-7: Multi-Channel Architecture | High | Medium - Flexibility |
| P2 | GAP-8: Reconnection Support | Medium | Medium - Reliability |
| P2 | GAP-9: Missing Event Types | Low | Medium - Feature completeness |
| P2 | GAP-10: Connection State | Low | Medium - Observability |
| P3 | GAP-11: Candle Helpers | Low | Medium - Usability |
| P3 | GAP-12: Channel State | Low | Low - Internal tracking |
| P3 | GAP-13: Error 1009 | Low | Low - Better error messages |
| P3 | GAP-14: Configuration | Medium | Low - Flexibility |
| P3 | GAP-15: Typed Filtering | Medium | Low - API ergonomics |

---

## Recommended Implementation Phases

### Phase 1: Critical Protocol Fixes (P0)
1. Implement FEED_SETUP message sending
2. Add proactive keepalive task (30-second interval)
3. Implement COMPACT data format parsing
4. Fix subscription message format

### Phase 2: Reliability Improvements (P1)
1. Add AUTH_STATE verification
2. Add CHANNEL_OPENED verification
3. Implement connection state tracking
4. Add reconnection support with backoff

### Phase 3: Feature Completeness (P2)
1. Add missing event types (TimeAndSale, TradeETH, Underlying)
2. Implement multi-channel architecture
3. Add candle subscription helpers

### Phase 4: API Enhancements (P3)
1. Add configuration options
2. Implement typed event filtering
3. Add channel state management
4. Improve error messages

---

## Test Verification Checklist

Each phase must pass these integration tests:

### Phase 1 Tests
- [ ] Connect and receive AUTH_STATE: AUTHORIZED
- [ ] Send FEED_SETUP and receive FEED_CONFIG
- [ ] Subscribe to Quote and receive COMPACT format data
- [ ] Verify keepalive prevents disconnection after 60 seconds
- [ ] Parse multiple event types correctly

### Phase 2 Tests
- [ ] Verify AUTH_STATE check fails gracefully with bad token
- [ ] Verify subscription waits for CHANNEL_OPENED
- [ ] Test reconnection after server disconnect
- [ ] Verify is_connected() state accuracy

### Phase 3 Tests
- [ ] Subscribe to all supported event types
- [ ] Test candle subscriptions with various intervals
- [ ] Verify multi-symbol subscriptions work correctly

### Phase 4 Tests
- [ ] Configure custom aggregation periods
- [ ] Test typed event filtering
- [ ] Verify error 1009 handling with large subscription

---

## Appendix A: COMPACT Format Field Mappings

### Quote Fields
```
Position 0: eventType
Position 1: eventSymbol
Position 2: bidPrice
Position 3: askPrice
Position 4: bidSize
Position 5: askSize
```

### Trade Fields
```
Position 0: eventType
Position 1: eventSymbol
Position 2: price
Position 3: dayVolume
Position 4: size
```

### Greeks Fields
```
Position 0: eventType
Position 1: eventSymbol
Position 2: volatility
Position 3: delta
Position 4: gamma
Position 5: theta
Position 6: rho
Position 7: vega
```

---

## Appendix B: Protocol Message Sequence

```
Client                                  Server
  |                                        |
  |---------- SETUP ---------------------->|
  |<--------- SETUP ----------------------|
  |<--------- AUTH_STATE: UNAUTHORIZED ----|
  |---------- AUTH ------------------------>|
  |<--------- AUTH_STATE: AUTHORIZED ------|
  |---------- CHANNEL_REQUEST ------------->|
  |<--------- CHANNEL_OPENED --------------|
  |---------- FEED_SETUP ----------------->|
  |<--------- FEED_CONFIG -----------------|
  |---------- FEED_SUBSCRIPTION ----------->|
  |<--------- FEED_DATA -------------------|
  |<--------- FEED_DATA -------------------|
  |---------- KEEPALIVE ------------------->| (every 30s)
  |<--------- KEEPALIVE -------------------|
  |         ...                            |
```

---

*Document Version: 1.0*
*Analysis Date: 2026-01-03*
*Analyst: Claude Code*
