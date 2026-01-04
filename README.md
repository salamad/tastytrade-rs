# tastytrade-rs

A production-grade Rust client for the [TastyTrade](https://tastyworks.com) brokerage API.

[![Crates.io](https://img.shields.io/crates/v/tastytrade-rs.svg)](https://crates.io/crates/tastytrade-rs)
[![Documentation](https://docs.rs/tastytrade-rs/badge.svg)](https://docs.rs/tastytrade-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Full API Coverage**: Accounts, orders, positions, balances, transactions, instruments, market data, watchlists
- **Real-time Streaming**: DXLink for market data (quotes, trades, greeks) and Account Streamer for notifications
- **Paginated Streaming**: Lazy iterators for memory-efficient pagination over large result sets
- **Type Safety**: Strongly-typed models with newtypes for compile-time guarantees
- **Async-first**: Built on Tokio for high-performance async I/O
- **Authentication**: OAuth2 and legacy session-based authentication with automatic token refresh
- **Order Builder**: Fluent API with compile-time validation for order construction
- **Production Ready**: Comprehensive error handling, retry logic, and connection management

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
tastytrade-rs = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `rustls-tls` | Use rustls for TLS (recommended) | Yes |
| `native-tls` | Use native OS TLS | No |
| `gzip` | Enable gzip compression | Yes |
| `streaming` | Enable WebSocket streaming | Yes |
| `blocking` | Enable blocking (sync) client | No |

## Quick Start

### Authentication

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    // Login with credentials
    let client = TastytradeClient::login(
        "your_username",
        "your_password",
        Environment::Sandbox, // Use Environment::Production for live trading
    ).await?;

    println!("Logged in successfully!");
    Ok(())
}
```

### Get Accounts and Balances

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // List all accounts
    let accounts = client.accounts().list().await?;

    for item in &accounts {
        let account = &item.account;
        println!("Account: {} ({})", account.account_number, account.nickname.as_deref().unwrap_or(""));

        // Get balance for this account
        let balance = client.balances().get(&account.account_number).await?;
        println!("  Net Liq: ${:?}", balance.net_liquidating_value);
        println!("  Buying Power: ${:?}", balance.equity_buying_power);
    }

    Ok(())
}
```

### Get Positions

```rust
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    let positions = client.balances().positions(&account).await?;

    for pos in positions {
        println!("{}: {} shares @ ${:?}",
            pos.symbol,
            pos.quantity,
            pos.average_open_price
        );
    }

    Ok(())
}
```

### Place an Order

```rust
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::models::{NewOrderBuilder, OrderType, TimeInForce, OrderAction, InstrumentType};
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Build a limit order to buy 10 shares of AAPL
    let order = NewOrderBuilder::new()
        .time_in_force(TimeInForce::Day)
        .order_type(OrderType::Limit)
        .price(dec!(150.00))
        .add_leg(InstrumentType::Equity, "AAPL", OrderAction::BuyToOpen, 10)
        .build()?;

    // Validate with dry run first
    let dry_run = client.orders().dry_run(&account, &order).await?;
    println!("Buying power effect: {:?}", dry_run.buying_power_effect);

    // Place the order
    let response = client.orders().place(&account, &order).await?;
    println!("Order placed! ID: {:?}", response.order.id);

    Ok(())
}
```

### Option Spread Order

```rust
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::models::{NewOrderBuilder, OrderType, TimeInForce, OrderAction, InstrumentType};
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Build a vertical spread
    let order = NewOrderBuilder::new()
        .time_in_force(TimeInForce::Day)
        .order_type(OrderType::Limit)
        .price(dec!(2.50)) // Net credit
        .add_leg(
            InstrumentType::EquityOption,
            "AAPL  240119C00150000", // Short call
            OrderAction::SellToOpen,
            1
        )
        .add_leg(
            InstrumentType::EquityOption,
            "AAPL  240119C00155000", // Long call
            OrderAction::BuyToOpen,
            1
        )
        .build()?;

    let response = client.orders().place(&account, &order).await?;
    println!("Spread order placed: {:?}", response.order.id);

    Ok(())
}
```

### Real-time Market Data Streaming

```rust
use tastytrade_rs::{TastytradeClient, Environment};
use tastytrade_rs::streaming::{Quote, Trade, DxEvent};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Create DXLink streamer for market data
    let mut streamer = client.streaming().dxlink().await?;

    // Subscribe to quotes for multiple symbols
    streamer.subscribe::<Quote>(&["AAPL", "SPY", "QQQ"]).await?;

    // Also subscribe to trades
    streamer.subscribe::<Trade>(&["AAPL"]).await?;

    // Process incoming events
    while let Some(event) = streamer.next().await {
        match event? {
            DxEvent::Quote(quote) => {
                println!("[QUOTE] {}: bid={:?} x {:?}, ask={:?} x {:?}",
                    quote.event_symbol,
                    quote.bid_price, quote.bid_size,
                    quote.ask_price, quote.ask_size
                );
            }
            DxEvent::Trade(trade) => {
                println!("[TRADE] {}: {} @ {:?}",
                    trade.event_symbol,
                    trade.size.unwrap_or_default(),
                    trade.price
                );
            }
            _ => {}
        }
    }

    Ok(())
}
```

### Account Activity Streaming

The account streamer provides real-time notifications for order fills, position changes, and balance updates - essential for trading bots.

```rust
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::streaming::{AccountNotification, ReconnectConfig};
use tastytrade_rs::models::OrderStatus;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Create account streamer with aggressive reconnection for 24/7 operation
    let mut streamer = client.streaming().account().await?
        .with_reconnect_config(ReconnectConfig::aggressive());

    // Subscribe to account updates
    streamer.subscribe(&account).await?;

    // Check connection state
    println!("Connected: {}", streamer.is_connected());
    println!("Subscribed accounts: {:?}", streamer.subscribed_accounts().await);

    // Process notifications
    while let Some(notification) = streamer.next().await {
        match notification? {
            AccountNotification::Order(order) => {
                // Use typed OrderStatus for pattern matching
                if order.is_filled() {
                    println!("Order {} FILLED at {:?}!",
                        order.order_id().unwrap_or_default(),
                        order.average_fill_price
                    );
                } else if order.is_rejected() {
                    println!("Order {} REJECTED", order.order_id().unwrap_or_default());
                } else if order.is_working() {
                    println!("Order {} is working ({:?})",
                        order.order_id().unwrap_or_default(),
                        order.status
                    );
                }

                // Check terminal state
                if order.is_terminal() {
                    println!("Order complete (terminal state)");
                }
            }
            AccountNotification::Position(pos) => {
                println!("[POSITION] {}: {:?} shares",
                    pos.symbol.unwrap_or_default(),
                    pos.quantity
                );
            }
            AccountNotification::Balance(bal) => {
                println!("[BALANCE] Net Liq: {:?}", bal.net_liquidating_value);
            }
            AccountNotification::Disconnected { reason } => {
                println!("Connection lost: {}", reason);
                // Auto-reconnect will handle this if configured
            }
            AccountNotification::Reconnected { accounts_restored } => {
                println!("Reconnected! {} accounts restored", accounts_restored);
            }
            AccountNotification::Heartbeat => {
                // Connection is alive
            }
            _ => {}
        }
    }

    Ok(())
}
```

### Get Option Chain

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Get nested option chain for AAPL
    let chain = client.instruments().option_chain_nested("AAPL").await?;

    println!("Option chain for: {}", chain.underlying_symbol);

    for expiration in chain.expirations {
        println!("\nExpiration: {}", expiration.expiration_date);

        for strike in expiration.strikes {
            println!("  Strike ${}: Call={} Put={}",
                strike.strike_price,
                strike.call.unwrap_or_default(),
                strike.put.unwrap_or_default()
            );
        }
    }

    Ok(())
}
```

### Market Metrics with IV by Expiration

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Get market metrics including IV data per expiration
    let metrics = client.metrics().get(&["SPY", "AAPL"]).await?;

    for metric in metrics {
        println!("{}: IV Rank={:?}, Liquidity Rating={:?}",
            metric.symbol,
            metric.effective_iv_rank(),
            metric.liquidity_rating
        );

        // Check liquidity
        if metric.has_good_liquidity() {
            println!("  Good liquidity for options trading");
        }

        // IV by expiration date
        for exp_iv in &metric.option_expiration_implied_volatilities {
            println!("  Expiration {:?}: IV={:?}",
                exp_iv.expiration_date,
                exp_iv.implied_volatility
            );
        }
    }

    Ok(())
}
```

### Option Greeks (Detailed Quotes)

Get detailed quotes with full Greeks for option contracts:

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Get detailed quote with Greeks for an option
    let quotes = client.market_data()
        .options_detailed(&["AAPL  250117C00200000"])
        .await?;

    for quote in quotes {
        println!("{}", quote.symbol);
        println!("  Bid: {:?} x {:?}", quote.bid, quote.bid_size);
        println!("  Ask: {:?} x {:?}", quote.ask, quote.ask_size);
        println!("  Greeks:");
        println!("    Delta: {:?}", quote.delta);
        println!("    Gamma: {:?}", quote.gamma);
        println!("    Theta: {:?}", quote.theta);
        println!("    Vega:  {:?}", quote.vega);
        println!("    Rho:   {:?}", quote.rho);
        println!("  IV: {:?}", quote.volatility);
        println!("  Theo Price: {:?}", quote.theo_price);
        println!("  Open Interest: {:?}", quote.open_interest);
    }

    Ok(())
}
```

### Market Sessions

Check if the market is open and get trading hours:

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Get current equity market session
    let session = client.market_time().current_equities_session().await?;

    println!("Market state: {:?}", session.state);
    println!("Opens at: {:?}", session.open_at);
    println!("Closes at: {:?}", session.close_at);

    // Check market status
    if session.is_open() {
        println!("Market is OPEN for trading!");
    } else if session.is_pre_market() {
        println!("Pre-market session");
    } else if session.is_after_hours() {
        println!("After-hours trading");
    } else {
        println!("Market is CLOSED");

        // Show next session
        if let Some(next) = &session.next_session {
            println!("Next session: {:?}", next.session_date);
            println!("  Opens at: {:?}", next.open_at);
        }
    }

    // Quick check if market is open
    let is_open = client.market_time().is_market_open().await?;
    println!("Is market open: {}", is_open);

    Ok(())
}
```

### Search Symbols

```rust
use tastytrade_rs::{TastytradeClient, Environment};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    let results = client.search().search("TSLA").await?;

    for result in results {
        println!("{}: {:?}", result.symbol, result.description);
    }

    Ok(())
}
```

### Transaction History

```rust
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::api::TransactionsQuery;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Get recent transactions
    let transactions = client.transactions().list(&account, None).await?;

    for txn in transactions.iter().take(10) {
        println!("{:?}: {:?} - ${:?}",
            txn.transaction_type,
            txn.symbol,
            txn.value
        );
    }

    Ok(())
}
```

### Paginated Streaming

For endpoints that return large result sets, use streaming iterators to lazily fetch pages on demand. This is more memory-efficient than loading all records at once.

```rust
use futures_util::StreamExt;
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::api::TransactionsQueryStream;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Stream ALL transactions lazily - pages are fetched as needed
    let mut stream = client.transactions().list_stream(&account, None);

    let mut count = 0;
    while let Some(result) = stream.next().await {
        let txn = result?;
        println!("{:?}: {:?} - ${:?}",
            txn.transaction_type,
            txn.symbol,
            txn.value
        );
        count += 1;
    }
    println!("Total transactions: {}", count);

    Ok(())
}
```

You can also filter and control batch size:

```rust
use futures_util::StreamExt;
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::api::TransactionsQueryStream;
use tastytrade_rs::models::TransactionType;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Stream only trades with custom page size
    let query = TransactionsQueryStream {
        transaction_type: Some(TransactionType::Trade),
        symbol: Some("AAPL".to_string()),
        per_page: Some(100), // Fetch 100 items per page
        ..Default::default()
    };

    let mut stream = client.transactions().list_stream(&account, Some(query));

    while let Some(result) = stream.next().await {
        let txn = result?;
        println!("Trade: {:?} @ {:?}", txn.symbol, txn.value);
    }

    Ok(())
}
```

Stream orders the same way:

```rust
use futures_util::StreamExt;
use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::api::OrdersQueryStream;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;
    let account = AccountNumber::new("5WV12345");

    // Stream all historical orders
    let mut stream = client.orders().list_stream(&account, None);

    while let Some(result) = stream.next().await {
        let order = result?;
        println!("Order {}: {:?}", order.id, order.status);
    }

    Ok(())
}
```

### Watchlists

```rust
use tastytrade_rs::{TastytradeClient, Environment};
use tastytrade_rs::models::WatchlistEntry;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let client = TastytradeClient::login("user", "pass", Environment::Sandbox).await?;

    // Get public watchlists
    let public = client.watchlists().public().await?;
    for list in &public {
        println!("Public: {}", list.name);
    }

    // Get user's watchlists
    let mine = client.watchlists().list().await?;
    for list in &mine {
        println!("My list: {}", list.name);
    }

    // Create a new watchlist
    let entries = vec![
        WatchlistEntry::equity("AAPL"),
        WatchlistEntry::equity("MSFT"),
        WatchlistEntry::equity("GOOGL"),
    ];

    let new_list = client.watchlists().create("Tech Giants", entries).await?;
    println!("Created: {}", new_list.name);

    Ok(())
}
```

## Error Handling

The crate provides comprehensive error types:

```rust
use tastytrade_rs::{TastytradeClient, Environment, Error};

#[tokio::main]
async fn main() {
    match TastytradeClient::login("user", "wrong_password", Environment::Sandbox).await {
        Ok(client) => println!("Connected!"),
        Err(Error::Authentication(msg)) => {
            eprintln!("Login failed: {}", msg);
        }
        Err(Error::Api { status, message, .. }) => {
            eprintln!("API error {}: {}", status, message);
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }
}
```

## Configuration

### Custom Client Configuration

```rust
use tastytrade_rs::{TastytradeClient, ClientConfig, RetryConfig, Environment};
use std::time::Duration;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    let config = ClientConfig::builder()
        .environment(Environment::Sandbox)
        .timeout(Duration::from_secs(30))
        .retry(RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
        })
        .build();

    let client = TastytradeClient::with_config("user", "pass", config).await?;

    Ok(())
}
```

## API Coverage

| Module | Endpoints | Status |
|--------|-----------|--------|
| Authentication | Login, OAuth, Token Refresh | Complete |
| Accounts | List, Get, Search | Complete |
| Balances | Get, Snapshots, History | Complete |
| Positions | List, Get | Complete |
| Orders | Place, Cancel, Replace, Dry Run, Stream | Complete |
| Instruments | Equities, Options, Futures, Crypto | Complete |
| Market Data | Quotes, Detailed Quotes with Greeks | Complete |
| Market Metrics | IV Rank, Liquidity, IV by Expiration | Complete |
| Market Time | Equity Sessions, Market Hours, State | Complete |
| Option Chains | Flat, Nested, Compact | Complete |
| Transactions | List, Get, Fees, Stream | Complete |
| Watchlists | Public, User, CRUD | Complete |
| Search | Symbol Search | Complete |
| DXLink Streaming | Quotes, Trades, Greeks, Summary | Complete |
| Account Streaming | Orders, Positions, Balances | Complete |
| Paginated Streaming | Lazy iteration over large result sets | Complete |

## Environments

| Environment | Base URL | Use Case |
|------------|----------|----------|
| `Production` | `api.tastyworks.com` | Live trading with real money |
| `Sandbox` | `api.cert.tastyworks.com` | Paper trading and development |

Always use `Environment::Sandbox` for development and testing!

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This is an unofficial client library. It is not affiliated with, maintained, authorized, endorsed, or sponsored by TastyTrade or any of its affiliates. Use at your own risk.

Trading involves significant risk of loss. Past performance is not indicative of future results. Always test thoroughly in the sandbox environment before using with real funds.
