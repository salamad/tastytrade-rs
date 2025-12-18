//! Real-time quote streaming example.
//!
//! This example demonstrates how to stream real-time market data using DXLink.
//!
//! Run with: cargo run --example stream_quotes --features streaming

use tastytrade_rs::{TastytradeClient, Environment};
use tastytrade_rs::streaming::{Quote, Trade, DxEvent};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get credentials from environment variables
    let username = std::env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME environment variable required");
    let password = std::env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD environment variable required");

    println!("Connecting to TastyTrade...");

    // Login
    let client = TastytradeClient::login(
        &username,
        &password,
        Environment::Sandbox,
    ).await?;

    println!("Creating DXLink streamer...");

    // Create market data streamer
    let mut streamer = client.streaming().dxlink().await?;

    // Subscribe to quotes for popular symbols
    let symbols = ["AAPL", "SPY", "QQQ", "TSLA", "NVDA"];
    println!("Subscribing to quotes for: {:?}", symbols);
    streamer.subscribe::<Quote>(&symbols).await?;

    // Also subscribe to trades for AAPL
    println!("Subscribing to trades for: AAPL");
    streamer.subscribe::<Trade>(&["AAPL"]).await?;

    println!("\nStreaming market data (Ctrl+C to stop)...\n");

    // Process incoming events
    let mut count = 0;
    while let Some(event) = streamer.next().await {
        match event? {
            DxEvent::Quote(quote) => {
                println!(
                    "[QUOTE] {}: bid=${:?} x {:?}, ask=${:?} x {:?}",
                    quote.event_symbol,
                    quote.bid_price,
                    quote.bid_size,
                    quote.ask_price,
                    quote.ask_size
                );
            }
            DxEvent::Trade(trade) => {
                println!(
                    "[TRADE] {}: {:?} shares @ ${:?}",
                    trade.event_symbol,
                    trade.size,
                    trade.price
                );
            }
            _ => {}
        }

        count += 1;
        // Stop after 100 events for demo purposes
        if count >= 100 {
            println!("\nReceived 100 events, stopping...");
            break;
        }
    }

    // Clean up
    streamer.close().await?;
    println!("Done!");
    Ok(())
}
