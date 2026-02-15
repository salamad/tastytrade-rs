//! Fetch market data for AAPL equity and an AAPL option.
//!
//! Run with: source .env.production && cargo run --example fetch_market_data

use tastytrade_rs::{TastytradeClient, Environment};
use tastytrade_rs::models::InstrumentType;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    tracing_subscriber::fmt::init();

    let client_secret = std::env::var("TASTYTRADE_CLIENT_SECRET")
        .expect("TASTYTRADE_CLIENT_SECRET required");
    let refresh_token = std::env::var("TASTYTRADE_REFRESH_TOKEN")
        .expect("TASTYTRADE_REFRESH_TOKEN required");

    println!("Authenticating via OAuth...");
    let client = TastytradeClient::from_oauth(
        &client_secret,
        &refresh_token,
        Environment::Production,
    ).await?;
    println!("Authenticated successfully!\n");

    // 1. Fetch equity market data for AAPL
    println!("=== AAPL Equity Market Data ===");
    let aapl = client.market_data().get("AAPL", InstrumentType::Equity).await?;
    println!("Symbol:       {}", aapl.symbol);
    println!("Bid:          {:?}", aapl.bid);
    println!("Ask:          {:?}", aapl.ask);
    println!("Last:         {:?}", aapl.last);
    println!("Mark:         {:?}", aapl.mark);
    println!("Volume:       {:?}", aapl.volume);
    println!("Prev Close:   {:?}", aapl.prev_close);
    println!("Net Change:   {:?}", aapl.net_change);
    println!("% Change:     {:?}", aapl.percent_change);
    println!("High:         {:?}", aapl.high);
    println!("Low:          {:?}", aapl.low);
    println!("52w High:     {:?}", aapl.week_52_high);
    println!("52w Low:      {:?}", aapl.week_52_low);
    println!("Spread:       {:?}", aapl.spread());
    println!();

    // 2. Fetch option market data
    let option_symbol = "AAPL  260223P00255000";
    println!("=== AAPL Option Market Data ===");
    println!("Symbol: {}", option_symbol);
    let opt = client.market_data().get(option_symbol, InstrumentType::EquityOption).await?;
    println!("Bid:          {:?}", opt.bid);
    println!("Ask:          {:?}", opt.ask);
    println!("Last:         {:?}", opt.last);
    println!("Mark:         {:?}", opt.mark);
    println!("Volume:       {:?}", opt.volume);
    println!("Open Int:     {:?}", opt.open_interest);
    println!("Impl Vol:     {:?}", opt.implied_volatility);
    println!("Spread:       {:?}", opt.spread());
    println!();

    // 3. Also try detailed quotes with Greeks for the option
    println!("=== AAPL Option Detailed Quote (with Greeks) ===");
    let detailed = client.market_data().options_detailed(&[option_symbol]).await?;
    if let Some(dq) = detailed.first() {
        println!("Symbol:       {}", dq.symbol);
        println!("Bid:          {:?}", dq.bid);
        println!("Ask:          {:?}", dq.ask);
        println!("Mark:         {:?}", dq.mark);
        println!("Delta:        {:?}", dq.delta);
        println!("Gamma:        {:?}", dq.gamma);
        println!("Theta:        {:?}", dq.theta);
        println!("Vega:         {:?}", dq.vega);
        println!("Rho:          {:?}", dq.rho);
        println!("Volatility:   {:?}", dq.volatility);
        println!("Theo Price:   {:?}", dq.theo_price);
        println!("Open Int:     {:?}", dq.open_interest);
    } else {
        println!("(No detailed quote returned — endpoint may not support this in current env)");
    }

    println!("\nDone!");
    Ok(())
}
