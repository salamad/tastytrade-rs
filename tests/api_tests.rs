//! Comprehensive Integration Tests for tastytrade-rs
//!
//! This test suite covers all REST API endpoints and WebSocket streaming features.
//!
//! Environment variables required:
//! - TASTYTRADE_USERNAME: Username for authentication
//! - TASTYTRADE_PASSWORD: Password for authentication
//! - TASTYTRADE_ACCOUNT: Account number for testing
//!
//! Optional environment variables:
//! - TASTYTRADE_ENVIRONMENT: "sandbox" (default) or "production"
//!
//! Run with: cargo test --test api_tests -- --test-threads=1
//!
//! To run against production (excluding order placement tests):
//! TASTYTRADE_ENVIRONMENT=production cargo test --test api_tests -- --skip test_place_and_cancel_order

use std::env;
use std::sync::Once;
use std::time::Duration;

use chrono::Utc;
use futures_util::StreamExt;
use rust_decimal_macros::dec;
use tokio::time::timeout;
use tracing_subscriber::EnvFilter;

use tastytrade_rs::prelude::*;
use tastytrade_rs::api::{
    OrdersQuery, OrdersQueryStream, TransactionsQuery, TransactionsQueryStream,
};
use tastytrade_rs::streaming::{
    AccountNotification, DxEvent, Quote, Trade, Summary, Profile,
};
use tastytrade_rs::RetryConfig;

static INIT: Once = Once::new();

/// Initialize logging for tests
fn init_logging() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_test_writer()
            .try_init()
            .ok();
    });
}

/// Get environment variables for testing
fn get_test_credentials() -> (String, String, String) {
    let username = env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME must be set");
    let password = env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD must be set");
    let account = env::var("TASTYTRADE_ACCOUNT")
        .expect("TASTYTRADE_ACCOUNT must be set");
    (username, password, account)
}

/// Get the environment to use for testing
fn get_test_environment() -> Environment {
    match env::var("TASTYTRADE_ENVIRONMENT")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "production" | "prod" => {
            tracing::warn!("Running tests against PRODUCTION environment");
            Environment::Production
        }
        _ => Environment::Sandbox,
    }
}

/// Create an authenticated client for testing
async fn create_client() -> TastytradeClient {
    init_logging();
    let (username, password, _) = get_test_credentials();
    let environment = get_test_environment();

    TastytradeClient::login(&username, &password, environment)
        .await
        .expect("Failed to create client")
}

/// Get the test account number
fn get_test_account() -> AccountNumber {
    let (_, _, account) = get_test_credentials();
    AccountNumber::new(&account)
}

// ============================================================================
// ACCOUNTS SERVICE TESTS
// ============================================================================

mod accounts_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_customer_profile() {
        let client = create_client().await;

        let customer = client.accounts().me().await;
        assert!(customer.is_ok(), "Should get customer profile: {:?}", customer);

        let customer = customer.unwrap();
        // Username and email might be empty in some sandbox accounts
        tracing::info!(
            "Customer: id={:?}, username={}, email={}, external_id={}",
            customer.id,
            customer.username,
            customer.email,
            customer.external_id
        );
        // At least one identifier should be present
        assert!(
            !customer.email.is_empty() || !customer.external_id.is_empty() || customer.id.is_some(),
            "At least one identifier should be present"
        );
    }

    #[tokio::test]
    async fn test_list_accounts() {
        let client = create_client().await;

        let accounts = client.accounts().list().await;
        assert!(accounts.is_ok(), "Should list accounts: {:?}", accounts);

        let accounts = accounts.unwrap();
        assert!(!accounts.is_empty(), "Should have at least one account");

        for item in &accounts {
            tracing::info!(
                "Account: {} ({:?})",
                item.account.account_number,
                item.account.margin_or_cash
            );
        }
    }

    #[tokio::test]
    async fn test_get_specific_account() {
        let client = create_client().await;
        let account_number = get_test_account();

        let account = client.accounts().get(&account_number).await;
        assert!(account.is_ok(), "Should get specific account: {:?}", account);

        let account = account.unwrap();
        assert_eq!(
            account.account_number,
            account_number.as_str(),
            "Account number should match"
        );

        tracing::info!(
            "Account details: {} - Type: {:?}, Day Trader: {:?}",
            account.account_number,
            account.margin_or_cash,
            account.day_trader_status
        );
    }

    #[tokio::test]
    async fn test_get_invalid_account() {
        let client = create_client().await;
        let invalid_account = AccountNumber::new("INVALID123");

        let result = client.accounts().get(&invalid_account).await;
        assert!(result.is_err(), "Should fail for invalid account");
    }
}

// ============================================================================
// BALANCES SERVICE TESTS
// ============================================================================

mod balances_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_account_balance() {
        let client = create_client().await;
        let account = get_test_account();

        let balance = client.balances().get(&account).await;
        assert!(balance.is_ok(), "Should get account balance: {:?}", balance);

        let balance = balance.unwrap();
        tracing::info!(
            "Balance - Net Liq: {:?}, Cash: {:?}, Buying Power: {:?}",
            balance.net_liquidating_value,
            balance.cash_balance,
            balance.equity_buying_power
        );
    }

    #[tokio::test]
    async fn test_get_positions() {
        let client = create_client().await;
        let account = get_test_account();

        let positions = client.balances().positions(&account).await;
        assert!(positions.is_ok(), "Should get positions: {:?}", positions);

        let positions = positions.unwrap();
        tracing::info!("Found {} positions", positions.len());

        for pos in &positions {
            tracing::info!(
                "Position: {} - Qty: {:?} {:?}, Avg Price: {:?}",
                pos.symbol,
                pos.quantity,
                pos.quantity_direction,
                pos.average_open_price
            );
        }
    }

    #[tokio::test]
    async fn test_get_balance_snapshots() {
        let client = create_client().await;
        let account = get_test_account();

        // Get snapshots for the last 30 days
        let end_date = Utc::now().date_naive();
        let start_date = end_date - chrono::Duration::days(30);

        let snapshots = client.balances()
            .snapshots(&account, Some(start_date), Some(end_date))
            .await;
        assert!(snapshots.is_ok(), "Should get balance snapshots: {:?}", snapshots);

        let snapshots = snapshots.unwrap();
        tracing::info!("Found {} balance snapshots", snapshots.len());
    }

    #[tokio::test]
    async fn test_get_balance_snapshots_no_dates() {
        let client = create_client().await;
        let account = get_test_account();

        let snapshots = client.balances()
            .snapshots(&account, None, None)
            .await;
        assert!(snapshots.is_ok(), "Should get balance snapshots without dates: {:?}", snapshots);
    }

    #[tokio::test]
    async fn test_get_net_liq_history() {
        let client = create_client().await;
        let account = get_test_account();

        // Test with different time periods
        for time_back in &[Some("1d"), Some("1m"), Some("1y"), None] {
            let history = client.balances()
                .net_liq_history(&account, *time_back)
                .await;

            // Net liq history may not be available in sandbox
            match &history {
                Ok(items) => {
                    tracing::info!(
                        "Net liq history (time_back={:?}): {} data points",
                        time_back,
                        items.len()
                    );
                }
                Err(tastytrade_rs::Error::NotFound(_)) => {
                    tracing::warn!("Net liq history endpoint not available in sandbox");
                    return; // Skip remaining time periods if endpoint not available
                }
                Err(e) => {
                    panic!("Unexpected error getting net liq history for {:?}: {:?}", time_back, e);
                }
            }
        }
    }
}

// ============================================================================
// ORDERS SERVICE TESTS
// ============================================================================

mod orders_tests {
    use super::*;

    #[tokio::test]
    async fn test_list_orders() {
        let client = create_client().await;
        let account = get_test_account();

        let orders = client.orders().list(&account, None).await;
        assert!(orders.is_ok(), "Should list orders: {:?}", orders);

        let orders = orders.unwrap();
        tracing::info!("Found {} orders", orders.len());

        for order in orders.iter().take(5) {
            tracing::info!(
                "Order {:?}: {:?} - {:?}",
                order.id,
                order.order_type,
                order.status
            );
        }
    }

    #[tokio::test]
    async fn test_list_orders_with_filters() {
        let client = create_client().await;
        let account = get_test_account();

        let query = OrdersQuery {
            status: Some(vec![OrderStatus::Filled]),
            per_page: Some(10),
            ..Default::default()
        };

        let orders = client.orders().list(&account, Some(query)).await;
        assert!(orders.is_ok(), "Should list filtered orders: {:?}", orders);

        let orders = orders.unwrap();
        tracing::info!("Found {} filled orders", orders.len());
    }

    #[tokio::test]
    async fn test_list_live_orders() {
        let client = create_client().await;
        let account = get_test_account();

        let orders = client.orders().live(&account).await;
        assert!(orders.is_ok(), "Should list live orders: {:?}", orders);

        let orders = orders.unwrap();
        tracing::info!("Found {} live orders", orders.len());
    }

    #[tokio::test]
    async fn test_dry_run_equity_order() {
        let client = create_client().await;
        let account = get_test_account();

        // Build a simple equity limit order
        let order = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(dec!(100.00))
            .price_effect(PriceEffect::Debit)
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(1)))
            .build()
            .expect("Should build order");

        let dry_run = client.orders().dry_run(&account, order).await;
        assert!(dry_run.is_ok(), "Should dry run order: {:?}", dry_run);

        let dry_run = dry_run.unwrap();
        tracing::info!(
            "Dry run - Buying power effect: {:?}, Fees: {:?}",
            dry_run.buying_power_effect,
            dry_run.fee_calculation
        );
    }

    #[tokio::test]
    async fn test_dry_run_market_order() {
        let client = create_client().await;
        let account = get_test_account();

        let order = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Market)
            .add_leg(OrderLeg::buy_equity("SPY", dec!(1)))
            .build()
            .expect("Should build market order");

        let dry_run = client.orders().dry_run(&account, order).await;
        assert!(dry_run.is_ok(), "Should dry run market order: {:?}", dry_run);
    }

    #[tokio::test]
    async fn test_order_stream_pagination() {
        let client = create_client().await;
        let account = get_test_account();

        let query = OrdersQueryStream {
            per_page: Some(10),
            ..Default::default()
        };

        let mut stream = client.orders().list_stream(&account, Some(query));
        let mut count = 0;

        // Collect up to 25 orders to test pagination
        while let Some(result) = stream.next().await {
            match result {
                Ok(_order) => {
                    count += 1;
                    if count >= 25 {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Error in order stream: {:?}", e);
                    break;
                }
            }
        }

        tracing::info!("Streamed {} orders", count);
    }

    #[tokio::test]
    async fn test_place_and_cancel_order() {
        let client = create_client().await;
        let account = get_test_account();

        // Place a very low limit order that won't execute
        let order = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(dec!(1.00)) // Very low price, won't execute
            .price_effect(PriceEffect::Debit)
            .add_leg(OrderLeg::buy_equity("AAPL", dec!(1)))
            .build()
            .expect("Should build order");

        // Place the order
        let place_result = client.orders().place(&account, order).await;

        match place_result {
            Ok(response) => {
                tracing::info!("Placed order: {:?}", response.order.id);

                // Cancel the order if we have an ID
                if let Some(id) = &response.order.id {
                    let order_id = OrderId::new(id);
                    let cancel_result = client.orders().cancel(&account, &order_id).await;

                    match cancel_result {
                        Ok(cancelled) => {
                            tracing::info!("Cancelled order: {:?}", cancelled.status);
                            assert!(
                                cancelled.status == Some(OrderStatus::Cancelled) ||
                                cancelled.status == Some(OrderStatus::CancelRequested),
                                "Order should be cancelled"
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Could not cancel order: {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                // Order placement might fail in sandbox for various reasons
                tracing::warn!("Could not place order (expected in some cases): {:?}", e);
            }
        }
    }
}

// ============================================================================
// INSTRUMENTS SERVICE TESTS
// ============================================================================

mod instruments_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_equities() {
        let client = create_client().await;

        let symbols = &["AAPL", "TSLA", "GOOGL", "MSFT", "AMZN"];
        let equities = client.instruments().equities(symbols).await;
        assert!(equities.is_ok(), "Should get equities: {:?}", equities);

        let equities = equities.unwrap();
        assert!(!equities.is_empty(), "Should return equities");

        for equity in &equities {
            tracing::info!(
                "Equity: {} - {:?}, Listed: {:?}",
                equity.symbol,
                equity.description,
                equity.listed_market
            );
        }
    }

    #[tokio::test]
    async fn test_get_single_equity() {
        let client = create_client().await;

        let equity = client.instruments().equity("AAPL").await;
        assert!(equity.is_ok(), "Should get single equity: {:?}", equity);

        let equity = equity.unwrap();
        assert_eq!(equity.symbol, "AAPL");
        tracing::info!("AAPL: {:?}", equity.description);
    }

    #[tokio::test]
    async fn test_get_option_chain() {
        let client = create_client().await;

        let chain = client.instruments().option_chain("AAPL").await;
        assert!(chain.is_ok(), "Should get option chain: {:?}", chain);

        let chain = chain.unwrap();
        tracing::info!(
            "AAPL option chain: {} expirations",
            chain.expirations.len()
        );

        // Check first expiration
        if let Some(exp) = chain.expirations.first() {
            tracing::info!(
                "First expiration: {:?} with {} strikes",
                exp.expiration_date,
                exp.strikes.len()
            );
        }
    }

    #[tokio::test]
    async fn test_get_equity_options() {
        let client = create_client().await;

        // First get the option chain to find valid option symbols
        let chain = client.instruments().option_chain("SPY").await;
        if let Ok(chain) = chain {
            if let Some(exp) = chain.expirations.first() {
                if let Some(strike) = exp.strikes.first() {
                    // strike.call is Option<String>
                    if let Some(call_symbol) = &strike.call {
                        let options = client.instruments()
                            .equity_options(&[call_symbol.as_str()])
                            .await;
                        assert!(
                            options.is_ok(),
                            "Should get equity options: {:?}",
                            options
                        );

                        let options = options.unwrap();
                        if !options.is_empty() {
                            tracing::info!(
                                "Option: {} - Strike: {:?}, Exp: {:?}",
                                options[0].symbol,
                                options[0].strike_price,
                                options[0].expiration_date
                            );
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn test_get_futures() {
        let client = create_client().await;

        // Get all available futures
        let futures = client.instruments().futures(None).await;
        assert!(futures.is_ok(), "Should get futures: {:?}", futures);

        let futures = futures.unwrap();
        tracing::info!("Found {} futures contracts", futures.len());

        // Log first few
        for future in futures.iter().take(5) {
            tracing::info!(
                "Future: {} - {:?}",
                future.symbol,
                future.description
            );
        }
    }

    #[tokio::test]
    async fn test_get_specific_futures() {
        let client = create_client().await;

        // Try to get specific E-mini S&P futures (common symbol)
        let futures = client.instruments().futures(Some(&["/ES"])).await;

        match futures {
            Ok(futures) => {
                tracing::info!("Found {} /ES futures", futures.len());
            }
            Err(e) => {
                tracing::warn!("Could not get /ES futures: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_cryptocurrencies() {
        let client = create_client().await;

        let cryptos = client.instruments().cryptocurrencies().await;
        assert!(cryptos.is_ok(), "Should get cryptocurrencies: {:?}", cryptos);

        let cryptos = cryptos.unwrap();
        tracing::info!("Found {} cryptocurrencies", cryptos.len());

        for crypto in &cryptos {
            tracing::info!(
                "Crypto: {} - {:?}",
                crypto.symbol,
                crypto.description
            );
        }
    }

    #[tokio::test]
    async fn test_get_single_cryptocurrency() {
        let client = create_client().await;

        let crypto = client.instruments().cryptocurrency("BTC/USD").await;

        match crypto {
            Ok(crypto) => {
                tracing::info!("BTC/USD: {:?}", crypto.description);
            }
            Err(e) => {
                // Crypto might not be available in sandbox
                tracing::warn!("Could not get BTC/USD: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_invalid_equity_symbol() {
        let client = create_client().await;

        let result = client.instruments().equity("INVALIDXYZ123").await;
        assert!(result.is_err(), "Should fail for invalid symbol");
    }
}

// ============================================================================
// MARKET DATA SERVICE TESTS
// ============================================================================

mod market_data_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_equity_quote() {
        let client = create_client().await;

        let quote = client.market_data()
            .get("AAPL", InstrumentType::Equity)
            .await;
        assert!(quote.is_ok(), "Should get equity quote: {:?}", quote);

        let quote = quote.unwrap();
        tracing::info!(
            "AAPL - Bid: {:?}, Ask: {:?}, Last: {:?}",
            quote.bid,
            quote.ask,
            quote.last
        );
    }

    #[tokio::test]
    async fn test_get_multiple_equity_quotes() {
        let client = create_client().await;

        let symbols = &["AAPL", "TSLA", "GOOGL", "MSFT"];
        let quotes = client.market_data()
            .get_many(symbols, InstrumentType::Equity)
            .await;
        assert!(quotes.is_ok(), "Should get multiple quotes: {:?}", quotes);

        let quotes = quotes.unwrap();
        // Some symbols may not be available in sandbox, so just check we got at least one
        assert!(!quotes.is_empty(), "Should return at least one quote");
        tracing::info!(
            "Requested {} symbols, got {} quotes",
            symbols.len(),
            quotes.len()
        );

        for quote in &quotes {
            tracing::info!(
                "{} - Bid: {:?}, Ask: {:?}",
                quote.symbol,
                quote.bid,
                quote.ask
            );
        }
    }

    #[tokio::test]
    async fn test_equity_quotes_convenience() {
        let client = create_client().await;

        let quotes = client.market_data()
            .equities(&["SPY", "QQQ", "IWM"])
            .await;
        assert!(quotes.is_ok(), "Should get equity quotes: {:?}", quotes);
    }

    #[tokio::test]
    async fn test_option_quotes() {
        let client = create_client().await;

        // First get a valid option symbol
        let chain = client.instruments().option_chain("SPY").await;
        if let Ok(chain) = chain {
            if let Some(exp) = chain.expirations.first() {
                if let Some(strike) = exp.strikes.first() {
                    // strike.call is Option<String>
                    if let Some(call_symbol) = &strike.call {
                        let quotes = client.market_data()
                            .options(&[call_symbol.as_str()])
                            .await;

                        match quotes {
                            Ok(quotes) => {
                                if !quotes.is_empty() {
                                    tracing::info!(
                                        "Option quote {}: Bid={:?}, Ask={:?}",
                                        quotes[0].symbol,
                                        quotes[0].bid,
                                        quotes[0].ask
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Could not get option quote: {:?}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn test_too_many_symbols_error() {
        let client = create_client().await;

        // Create 101 symbols to exceed the limit
        let symbols: Vec<String> = (0..101).map(|i| format!("SYM{}", i)).collect();
        let symbol_refs: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();

        let result = client.market_data()
            .get_many(&symbol_refs, InstrumentType::Equity)
            .await;

        assert!(result.is_err(), "Should fail with too many symbols");
    }
}

// ============================================================================
// TRANSACTIONS SERVICE TESTS
// ============================================================================

mod transactions_tests {
    use super::*;

    #[tokio::test]
    async fn test_list_transactions() {
        let client = create_client().await;
        let account = get_test_account();

        let transactions = client.transactions().list(&account, None).await;
        assert!(transactions.is_ok(), "Should list transactions: {:?}", transactions);

        let transactions = transactions.unwrap();
        tracing::info!("Found {} transactions", transactions.len());

        for txn in transactions.iter().take(5) {
            tracing::info!(
                "Transaction {:?}: {:?} - {:?}",
                txn.id,
                txn.transaction_type,
                txn.value
            );
        }
    }

    #[tokio::test]
    async fn test_list_transactions_with_filters() {
        let client = create_client().await;
        let account = get_test_account();

        let query = TransactionsQuery {
            transaction_type: Some(TransactionType::Trade),
            per_page: Some(10),
            ..Default::default()
        };

        let transactions = client.transactions()
            .list(&account, Some(query))
            .await;
        assert!(
            transactions.is_ok(),
            "Should list filtered transactions: {:?}",
            transactions
        );
    }

    #[tokio::test]
    async fn test_get_total_fees() {
        let client = create_client().await;
        let account = get_test_account();

        let fees = client.transactions().total_fees(&account).await;
        assert!(fees.is_ok(), "Should get total fees: {:?}", fees);

        let fees = fees.unwrap();
        tracing::info!(
            "Total fees - Regulatory: {:?}, Clearing: {:?}, Commissions: {:?}",
            fees.total_regulatory_fees,
            fees.total_clearing_fees,
            fees.total_commissions
        );
    }

    #[tokio::test]
    async fn test_transaction_stream_pagination() {
        let client = create_client().await;
        let account = get_test_account();

        let query = TransactionsQueryStream {
            per_page: Some(10),
            ..Default::default()
        };

        let mut stream = client.transactions().list_stream(&account, Some(query));
        let mut count = 0;

        // Collect up to 25 transactions
        while let Some(result) = stream.next().await {
            match result {
                Ok(_txn) => {
                    count += 1;
                    if count >= 25 {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Error in transaction stream: {:?}", e);
                    break;
                }
            }
        }

        tracing::info!("Streamed {} transactions", count);
    }
}

// ============================================================================
// WATCHLISTS SERVICE TESTS
// ============================================================================

mod watchlists_tests {
    use super::*;
    use tastytrade_rs::models::WatchlistEntry;

    #[tokio::test]
    async fn test_get_public_watchlists() {
        let client = create_client().await;

        let watchlists = client.watchlists().public().await;
        assert!(watchlists.is_ok(), "Should get public watchlists: {:?}", watchlists);

        let watchlists = watchlists.unwrap();
        tracing::info!("Found {} public watchlists", watchlists.len());

        for wl in watchlists.iter().take(5) {
            tracing::info!("Public watchlist: {}", wl.name);
        }
    }

    #[tokio::test]
    async fn test_list_user_watchlists() {
        let client = create_client().await;

        let watchlists = client.watchlists().list().await;
        assert!(watchlists.is_ok(), "Should list user watchlists: {:?}", watchlists);

        let watchlists = watchlists.unwrap();
        tracing::info!("Found {} user watchlists", watchlists.len());
    }

    #[tokio::test]
    async fn test_create_update_delete_watchlist() {
        let client = create_client().await;

        let watchlist_name = format!("test_watchlist_{}", Utc::now().timestamp());

        // Create watchlist
        let entries = vec![
            WatchlistEntry::equity("AAPL"),
            WatchlistEntry::equity("TSLA"),
            WatchlistEntry::equity("GOOGL"),
        ];

        let created = client.watchlists()
            .create(&watchlist_name, entries)
            .await;

        match created {
            Ok(watchlist) => {
                tracing::info!("Created watchlist: {}", watchlist.name);

                // Update watchlist
                let new_entries = vec![
                    WatchlistEntry::equity("AAPL"),
                    WatchlistEntry::equity("MSFT"),
                    WatchlistEntry::equity("AMZN"),
                    WatchlistEntry::equity("META"),
                ];

                let updated = client.watchlists()
                    .update(&watchlist_name, new_entries)
                    .await;

                match updated {
                    Ok(wl) => {
                        tracing::info!("Updated watchlist with {} entries", wl.watchlist_entries.len());
                    }
                    Err(e) => {
                        tracing::warn!("Could not update watchlist: {:?}", e);
                    }
                }

                // Get the watchlist
                let retrieved = client.watchlists().get(&watchlist_name).await;
                assert!(retrieved.is_ok(), "Should retrieve watchlist");

                // Delete watchlist
                let deleted = client.watchlists().delete(&watchlist_name).await;
                match deleted {
                    Ok(()) => {
                        tracing::info!("Deleted watchlist: {}", watchlist_name);
                    }
                    Err(e) => {
                        tracing::warn!("Could not delete watchlist: {:?}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Could not create watchlist: {:?}", e);
            }
        }
    }
}

// ============================================================================
// METRICS SERVICE TESTS
// ============================================================================

mod metrics_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_market_metrics() {
        let client = create_client().await;

        let metrics = client.metrics().get(&["AAPL", "SPY", "QQQ"]).await;

        // Metrics endpoint may not be available in sandbox
        match &metrics {
            Ok(items) => {
                tracing::info!("Got {} market metrics", items.len());
                for m in items {
                    tracing::info!(
                        "{} - IV Rank: {:?}, IV Percentile: {:?}",
                        m.symbol,
                        m.implied_volatility_index_rank,
                        m.implied_volatility_index_percentile
                    );
                }
            }
            Err(tastytrade_rs::Error::NotFound(_)) => {
                tracing::warn!("Market metrics endpoint not available in sandbox");
            }
            Err(e) => {
                panic!("Unexpected error getting market metrics: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_single_metric() {
        let client = create_client().await;

        let metric = client.metrics().get_one("AAPL").await;

        // Metrics endpoint may not be available in sandbox
        match &metric {
            Ok(m) => {
                tracing::info!(
                    "AAPL metrics - IV Rank: {:?}, Earnings: {:?}",
                    m.implied_volatility_index_rank,
                    m.earnings_date
                );
            }
            Err(tastytrade_rs::Error::NotFound(_)) => {
                tracing::warn!("Market metrics endpoint not available in sandbox");
            }
            Err(e) => {
                panic!("Unexpected error getting single metric: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_historical_volatility() {
        let client = create_client().await;

        let volatility = client.metrics().historical_volatility("AAPL").await;

        match volatility {
            Ok(data) => {
                tracing::info!("Found {} volatility data points", data.len());
                if let Some(first) = data.first() {
                    tracing::info!(
                        "First point: {:?} - IV: {:?}, HV: {:?}",
                        first.date,
                        first.implied_volatility,
                        first.historical_volatility
                    );
                }
            }
            Err(tastytrade_rs::Error::NotFound(_)) => {
                tracing::warn!("Historical volatility endpoint not available in sandbox");
            }
            Err(e) => {
                tracing::warn!("Could not get historical volatility: {:?}", e);
            }
        }
    }
}

// ============================================================================
// SEARCH SERVICE TESTS
// ============================================================================

mod search_tests {
    use super::*;

    #[tokio::test]
    async fn test_search_symbols() {
        let client = create_client().await;

        let results = client.search().search("AAPL").await;
        assert!(results.is_ok(), "Should search symbols: {:?}", results);

        let results = results.unwrap();
        assert!(!results.is_empty(), "Should find AAPL");

        for result in results.iter().take(5) {
            tracing::info!(
                "Search result: {} - {:?}",
                result.symbol,
                result.description
            );
        }
    }

    #[tokio::test]
    async fn test_search_by_name() {
        let client = create_client().await;

        let results = client.search().search("Apple").await;
        assert!(results.is_ok(), "Should search by name: {:?}", results);

        let results = results.unwrap();
        tracing::info!("Found {} results for 'Apple'", results.len());
    }

    #[tokio::test]
    async fn test_search_no_results() {
        let client = create_client().await;

        let results = client.search().search("XYZNONEXISTENT123").await;
        assert!(results.is_ok(), "Should handle no results gracefully");

        let results = results.unwrap();
        assert!(results.is_empty(), "Should return empty for nonexistent symbol");
    }
}

// ============================================================================
// DXLINK STREAMING TESTS
// ============================================================================

mod dxlink_streaming_tests {
    use super::*;

    #[tokio::test]
    async fn test_dxlink_connect_and_subscribe_quotes() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe to quotes
        let subscribe_result = streamer.subscribe::<Quote>(&["AAPL", "SPY"]).await;
        assert!(subscribe_result.is_ok(), "Should subscribe to quotes: {:?}", subscribe_result);

        // Wait for events with timeout
        let mut received_count = 0;
        let timeout_duration = Duration::from_secs(10);

        let result = timeout(timeout_duration, async {
            while let Some(event) = streamer.next().await {
                match event {
                    Ok(DxEvent::Quote(quote)) => {
                        tracing::info!(
                            "Quote: {} - Bid: {:?}, Ask: {:?}",
                            quote.event_symbol,
                            quote.bid_price,
                            quote.ask_price
                        );
                        received_count += 1;
                        if received_count >= 3 {
                            break;
                        }
                    }
                    Ok(other) => {
                        tracing::info!("Other event: {:?}", other);
                    }
                    Err(e) => {
                        tracing::warn!("Stream error: {:?}", e);
                        break;
                    }
                }
            }
        }).await;

        if result.is_err() {
            tracing::info!("Timeout reached after receiving {} events", received_count);
        }

        // Close the connection
        let _ = streamer.close().await;

        // Note: During market closure or in sandbox, we may not receive quotes
        // So we just log the result rather than asserting
        if received_count > 0 {
            tracing::info!("Successfully received {} quote events", received_count);
        } else {
            tracing::warn!(
                "No quote events received (market may be closed or sandbox limitation)"
            );
        }
    }

    #[tokio::test]
    async fn test_dxlink_subscribe_trades() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe to trades
        streamer.subscribe::<Trade>(&["AAPL"]).await
            .expect("Should subscribe to trades");

        // Wait for events
        let mut received = false;
        let timeout_duration = Duration::from_secs(15);

        let _ = timeout(timeout_duration, async {
            while let Some(event) = streamer.next().await {
                match event {
                    Ok(DxEvent::Trade(trade)) => {
                        tracing::info!(
                            "Trade: {} - Price: {:?}, Size: {:?}",
                            trade.event_symbol,
                            trade.price,
                            trade.size
                        );
                        received = true;
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::warn!("Stream error: {:?}", e);
                        break;
                    }
                }
            }
        }).await;

        let _ = streamer.close().await;

        // Trades might not happen during test, so just log
        if !received {
            tracing::info!("No trades received within timeout (expected during quiet periods)");
        }
    }

    #[tokio::test]
    async fn test_dxlink_subscribe_summary() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe to summary
        streamer.subscribe::<Summary>(&["AAPL", "SPY"]).await
            .expect("Should subscribe to summary");

        // Wait for events
        let mut received = false;
        let timeout_duration = Duration::from_secs(10);

        let _ = timeout(timeout_duration, async {
            while let Some(event) = streamer.next().await {
                match event {
                    Ok(DxEvent::Summary(summary)) => {
                        tracing::info!(
                            "Summary: {} - Open: {:?}, High: {:?}, Low: {:?}, Close: {:?}",
                            summary.event_symbol,
                            summary.day_open_price,
                            summary.day_high_price,
                            summary.day_low_price,
                            summary.day_close_price
                        );
                        received = true;
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::warn!("Stream error: {:?}", e);
                        break;
                    }
                }
            }
        }).await;

        let _ = streamer.close().await;

        if received {
            tracing::info!("Successfully received summary data");
        }
    }

    #[tokio::test]
    async fn test_dxlink_subscribe_profile() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe to profile
        streamer.subscribe::<Profile>(&["AAPL"]).await
            .expect("Should subscribe to profile");

        // Wait for events
        let timeout_duration = Duration::from_secs(10);

        let _ = timeout(timeout_duration, async {
            while let Some(event) = streamer.next().await {
                match event {
                    Ok(DxEvent::Profile(profile)) => {
                        tracing::info!(
                            "Profile: {} - Description: {:?}",
                            profile.event_symbol,
                            profile.description
                        );
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        tracing::warn!("Stream error: {:?}", e);
                        break;
                    }
                }
            }
        }).await;

        let _ = streamer.close().await;
    }

    #[tokio::test]
    async fn test_dxlink_unsubscribe() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe
        streamer.subscribe::<Quote>(&["AAPL"]).await
            .expect("Should subscribe");

        // Unsubscribe
        let unsub_result = streamer.unsubscribe::<Quote>(&["AAPL"]).await;
        assert!(unsub_result.is_ok(), "Should unsubscribe: {:?}", unsub_result);

        let _ = streamer.close().await;
    }

    #[tokio::test]
    async fn test_dxlink_multiple_subscriptions() {
        let client = create_client().await;

        let mut streamer = client.streaming().dxlink().await
            .expect("Should connect to DXLink");

        // Subscribe to multiple event types
        streamer.subscribe::<Quote>(&["AAPL"]).await
            .expect("Should subscribe to quotes");
        streamer.subscribe::<Trade>(&["AAPL"]).await
            .expect("Should subscribe to trades");
        streamer.subscribe::<Summary>(&["AAPL"]).await
            .expect("Should subscribe to summary");

        // Collect events
        let mut quote_received = false;
        let mut trade_received = false;
        let mut summary_received = false;

        let timeout_duration = Duration::from_secs(15);

        let _ = timeout(timeout_duration, async {
            while let Some(event) = streamer.next().await {
                match event {
                    Ok(DxEvent::Quote(_)) => quote_received = true,
                    Ok(DxEvent::Trade(_)) => trade_received = true,
                    Ok(DxEvent::Summary(_)) => summary_received = true,
                    Ok(_) => {}
                    Err(_) => break,
                }

                // Exit after receiving some events
                if quote_received && (trade_received || summary_received) {
                    break;
                }
            }
        }).await;

        let _ = streamer.close().await;

        tracing::info!(
            "Received - Quotes: {}, Trades: {}, Summary: {}",
            quote_received, trade_received, summary_received
        );
    }
}

// ============================================================================
// ACCOUNT STREAMER TESTS
// ============================================================================

mod account_streamer_tests {
    use super::*;

    #[tokio::test]
    async fn test_account_streamer_connect() {
        let client = create_client().await;

        let mut streamer = client.streaming().account().await
            .expect("Should connect to account streamer");

        let _ = streamer.close().await;
    }

    #[tokio::test]
    async fn test_account_streamer_subscribe() {
        let client = create_client().await;
        let account = get_test_account();

        let mut streamer = client.streaming().account().await
            .expect("Should connect to account streamer");

        // Subscribe to account updates
        let subscribe_result = streamer.subscribe(&account).await;
        assert!(subscribe_result.is_ok(), "Should subscribe to account: {:?}", subscribe_result);

        // Wait for confirmation or heartbeat
        let timeout_duration = Duration::from_secs(10);
        let mut received_notification = false;

        let _ = timeout(timeout_duration, async {
            while let Some(notification) = streamer.next().await {
                match notification {
                    Ok(AccountNotification::Heartbeat) => {
                        tracing::info!("Received heartbeat");
                        received_notification = true;
                        break;
                    }
                    Ok(AccountNotification::SubscriptionConfirmation { accounts }) => {
                        tracing::info!("Subscription confirmed for: {:?}", accounts);
                        received_notification = true;
                        break;
                    }
                    Ok(other) => {
                        tracing::info!("Received notification: {:?}", other);
                        received_notification = true;
                    }
                    Err(e) => {
                        tracing::warn!("Stream error: {:?}", e);
                        break;
                    }
                }
            }
        }).await;

        let _ = streamer.close().await;

        if received_notification {
            tracing::info!("Successfully received account notification");
        }
    }

    #[tokio::test]
    async fn test_account_streamer_unsubscribe() {
        let client = create_client().await;
        let account = get_test_account();

        let mut streamer = client.streaming().account().await
            .expect("Should connect to account streamer");

        // Subscribe
        streamer.subscribe(&account).await
            .expect("Should subscribe");

        // Unsubscribe
        let unsub_result = streamer.unsubscribe(&account).await;
        assert!(unsub_result.is_ok(), "Should unsubscribe: {:?}", unsub_result);

        let _ = streamer.close().await;
    }

    #[tokio::test]
    async fn test_account_streamer_quote_alerts() {
        let client = create_client().await;

        let mut streamer = client.streaming().account().await
            .expect("Should connect to account streamer");

        // Subscribe to quote alerts
        let subscribe_result = streamer.subscribe_quote_alerts().await;
        assert!(subscribe_result.is_ok(), "Should subscribe to quote alerts: {:?}", subscribe_result);

        let _ = streamer.close().await;
    }

    #[tokio::test]
    async fn test_account_streamer_heartbeat() {
        let client = create_client().await;
        let account = get_test_account();

        let mut streamer = client.streaming().account().await
            .expect("Should connect to account streamer");

        streamer.subscribe(&account).await
            .expect("Should subscribe");

        // Wait specifically for heartbeats
        let mut heartbeat_count = 0;
        let timeout_duration = Duration::from_secs(30);

        let _ = timeout(timeout_duration, async {
            while let Some(notification) = streamer.next().await {
                match notification {
                    Ok(AccountNotification::Heartbeat) => {
                        heartbeat_count += 1;
                        tracing::info!("Heartbeat #{}", heartbeat_count);
                        if heartbeat_count >= 2 {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }).await;

        let _ = streamer.close().await;

        tracing::info!("Received {} heartbeats", heartbeat_count);
    }
}

// ============================================================================
// CLIENT CONFIGURATION TESTS
// ============================================================================

mod client_config_tests {
    use super::*;

    #[tokio::test]
    async fn test_client_with_custom_config() {
        init_logging();
        let (username, password, _) = get_test_credentials();
        let environment = get_test_environment();

        let config = ClientConfig {
            timeout: Duration::from_secs(60),
            retry: RetryConfig {
                max_retries: 5,
                ..Default::default()
            },
            ..Default::default()
        };

        let client = TastytradeClient::login_with_config(
            &username,
            &password,
            environment,
            config,
        ).await;

        assert!(client.is_ok(), "Should create client with custom config: {:?}", client);
    }

    #[tokio::test]
    async fn test_client_session() {
        let client = create_client().await;

        // Session should be valid after login
        let customer = client.accounts().me().await;
        assert!(customer.is_ok(), "Session should be valid");
    }
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_credentials() {
        init_logging();
        let environment = get_test_environment();

        let result = TastytradeClient::login(
            "invalid_user",
            "invalid_password",
            environment,
        ).await;

        assert!(result.is_err(), "Should fail with invalid credentials");
    }

    #[tokio::test]
    async fn test_api_error_handling() {
        let client = create_client().await;
        let invalid_account = AccountNumber::new("NOTREAL123");

        // Should return an error for invalid account
        let result = client.balances().get(&invalid_account).await;
        assert!(result.is_err(), "Should return error for invalid account");

        if let Err(e) = result {
            tracing::info!("Expected error: {:?}", e);
        }
    }

    #[tokio::test]
    async fn test_invalid_order_dry_run() {
        // Create an invalid order (no legs)
        let order_result = NewOrderBuilder::new()
            .time_in_force(TimeInForce::Day)
            .order_type(OrderType::Limit)
            .price(dec!(100.00))
            .build();

        // Should fail to build without legs
        assert!(order_result.is_err(), "Should fail to build order without legs");
    }
}

// ============================================================================
// PAGINATION TESTS
// ============================================================================

mod pagination_tests {
    use super::*;

    #[tokio::test]
    async fn test_orders_pagination() {
        let client = create_client().await;
        let account = get_test_account();

        // Test with small page size
        let query = OrdersQuery {
            per_page: Some(5),
            page_offset: Some(0),
            ..Default::default()
        };

        let first_page = client.orders().list(&account, Some(query)).await;
        assert!(first_page.is_ok(), "Should get first page");

        let first_page = first_page.unwrap();

        if first_page.len() == 5 {
            // Try second page
            let query2 = OrdersQuery {
                per_page: Some(5),
                page_offset: Some(1),
                ..Default::default()
            };

            let second_page = client.orders().list(&account, Some(query2)).await;
            assert!(second_page.is_ok(), "Should get second page");
        }
    }

    #[tokio::test]
    async fn test_transactions_pagination() {
        let client = create_client().await;
        let account = get_test_account();

        let query = TransactionsQuery {
            per_page: Some(5),
            page_offset: Some(0),
            ..Default::default()
        };

        let first_page = client.transactions().list(&account, Some(query)).await;
        assert!(first_page.is_ok(), "Should get first page of transactions");
    }
}

// ============================================================================
// CONCURRENT REQUESTS TESTS
// ============================================================================

mod concurrent_tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_requests() {
        let client = create_client().await;
        let account = get_test_account();

        // Get service references
        let accounts_svc = client.accounts();
        let balances_svc = client.balances();
        let instruments_svc = client.instruments();
        let metrics_svc = client.metrics();

        // Run multiple requests concurrently
        let (accounts, balance, equities, metrics) = tokio::join!(
            accounts_svc.list(),
            balances_svc.get(&account),
            instruments_svc.equities(&["AAPL", "TSLA"]),
            metrics_svc.get(&["SPY"]),
        );

        assert!(accounts.is_ok(), "Accounts request should succeed");
        assert!(balance.is_ok(), "Balance request should succeed");
        assert!(equities.is_ok(), "Equities request should succeed");
        // Metrics may not be available in sandbox
        match &metrics {
            Ok(_) => tracing::info!("Metrics request succeeded"),
            Err(tastytrade_rs::Error::NotFound(_)) => {
                tracing::warn!("Metrics endpoint not available in sandbox");
            }
            Err(e) => panic!("Unexpected metrics error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_rapid_sequential_requests() {
        let client = create_client().await;

        // Make rapid sequential requests
        for i in 0..5 {
            let result = client.accounts().me().await;
            assert!(result.is_ok(), "Request {} should succeed", i);
        }
    }
}
