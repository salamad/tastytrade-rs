# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build the library
cargo build

# Run all tests (unit + doc tests)
cargo test

# Run a specific test
cargo test test_name

# Run tests in a specific module
cargo test models::tests

# Check without building
cargo check

# Run clippy lints
cargo clippy

# Build docs
cargo doc --open
```

## Architecture

This is a Rust client library for the TastyTrade brokerage API. The crate is async-first, built on Tokio.

### Core Structure

- **`client/`** - HTTP client layer
  - `TastytradeClient` is the main entry point, holding an `Arc<ClientInner>`
  - `ClientInner` manages reqwest HTTP client, session, and config
  - `PaginatedStream<T>` implements `futures::Stream` for lazy pagination

- **`api/`** - Service modules (one per API domain)
  - Each service (e.g., `OrdersService`, `AccountsService`) takes `Arc<ClientInner>`
  - Pattern: `client.orders().list(&account).await?`

- **`models/`** - Data types
  - `primitives.rs` - Newtypes (`AccountNumber`, `OrderId`, `Symbol`)
  - `enums.rs` - API enums with serde rename rules
  - Domain models: `account.rs`, `order.rs`, `balance.rs`, `instrument.rs`, etc.

- **`streaming/`** - WebSocket streams
  - `dxlink/` - DXLink market data (quotes, trades, greeks)
  - `account/` - Account notifications (orders, positions, balances)

- **`auth/`** - Session management with automatic token refresh

- **`error.rs`** - `Error` enum with `is_retryable()`, `is_auth_error()` helpers

### Key Patterns

**API Response Wrapping**: All API responses come wrapped in `{ "data": T, "context": ..., "pagination": ... }`. The `ApiResponse<T>` struct in `client/http.rs` handles this.

**Kebab-case JSON**: TastyTrade uses kebab-case for JSON fields. All models use `#[serde(rename_all = "kebab-case")]`.

**Monetary Values**: Use `rust_decimal::Decimal` for all prices/amounts. The crate uses `serde-with-str` for string-based decimal serialization.

**Paginated Endpoints**: Services offer both:
- `list()` - Returns `Vec<T>` (fetches first page only)
- `list_stream()` - Returns `PaginatedStream<T>` (lazily fetches all pages)

**Streaming Events**: DxLink events implement `DxEventTrait` and can be subscribed to by type:
```rust
streamer.subscribe::<Quote>(&["AAPL"]).await?;
```

### Feature Flags

- `streaming` (default) - Enables WebSocket streaming
- `rustls-tls` (default) - TLS via rustls
- `native-tls` - TLS via OS native
- `gzip` (default) - Response compression
- `blocking` - Sync client support

### Testing

Tests are primarily unit tests in each module. Integration tests would require API credentials. Doctests are used extensively for API examples.

## Environments

- `Environment::Production` - `api.tastyworks.com` (live trading)
- `Environment::Sandbox` - `api.cert.tastyworks.com` (paper trading)

Always default to Sandbox for development.
