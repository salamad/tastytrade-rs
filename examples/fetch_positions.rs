//! Fetch positions example.
//!
//! Demonstrates fetching all positions for each account, exercising
//! the Position deserialization including the Zero quantity-direction variant.
//!
//! Run with: source .env.production && cargo run --example fetch_positions

use tastytrade_rs::{AccountNumber, Environment, TastytradeClient};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    tracing_subscriber::fmt::init();

    let username = std::env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME environment variable required");
    let password = std::env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD environment variable required");

    let environment = match std::env::var("TASTYTRADE_ENV").as_deref() {
        Ok("sandbox") => Environment::Sandbox,
        _ => Environment::Production,
    };

    println!("Connecting to TastyTrade ({environment:?})...");

    let client = TastytradeClient::login(&username, &password, environment).await?;
    println!("Authenticated successfully.\n");

    let accounts = client.accounts().list().await?;

    for item in &accounts {
        let acct = &item.account;
        let account_number = AccountNumber::new(&acct.account_number);
        println!(
            "Account {} ({}):",
            acct.account_number,
            acct.nickname.as_deref().unwrap_or("no nickname")
        );

        let positions = client.balances().positions(&account_number).await?;
        println!("  {} position(s)", positions.len());

        for pos in &positions {
            println!(
                "    {:<30} {:>10} {:?}  qty={:<8} frozen={}",
                pos.symbol,
                format!("{:?}", pos.instrument_type),
                pos.quantity_direction,
                pos.quantity,
                pos.is_frozen,
            );
        }
        println!();
    }

    println!("Done!");
    Ok(())
}
