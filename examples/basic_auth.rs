//! Basic authentication example.
//!
//! This example demonstrates how to authenticate with the TastyTrade API
//! and retrieve account information.
//!
//! Run with: cargo run --example basic_auth

use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get credentials from environment variables
    let username = std::env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME environment variable required");
    let password = std::env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD environment variable required");

    println!("Connecting to TastyTrade sandbox...");

    // Login to sandbox environment
    let client = TastytradeClient::login(
        &username,
        &password,
        Environment::Sandbox,
    ).await?;

    println!("Successfully authenticated!");

    // List all accounts
    let accounts = client.accounts().list().await?;
    println!("\nFound {} account(s):", accounts.len());

    for item in &accounts {
        let account = &item.account;
        let account_number = AccountNumber::new(&account.account_number);
        println!(
            "  - {} ({})",
            account.account_number,
            account.nickname.as_deref().unwrap_or("No nickname")
        );

        // Get balance for this account
        let balance = client.balances().get(&account_number).await?;
        println!("    Net Liquidating Value: ${:?}", balance.net_liquidating_value);
        println!("    Cash Balance: ${:?}", balance.cash_balance);
        println!("    Equity Buying Power: ${:?}", balance.equity_buying_power);
    }

    println!("\nDone!");
    Ok(())
}
