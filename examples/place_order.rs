//! Order placement example.
//!
//! This example demonstrates how to place an order using the TastyTrade API.
//! It uses a dry run first to validate the order before actually placing it.
//!
//! Run with: cargo run --example place_order

use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
use tastytrade_rs::models::{NewOrderBuilder, OrderType, TimeInForce, OrderLeg, InstrumentType, OrderAction};
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> tastytrade_rs::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get credentials from environment variables
    let username = std::env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME environment variable required");
    let password = std::env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD environment variable required");
    let account_num = std::env::var("TASTYTRADE_ACCOUNT")
        .expect("TASTYTRADE_ACCOUNT environment variable required");

    println!("Connecting to TastyTrade sandbox...");

    // Login to sandbox environment (never use Production for examples!)
    let client = TastytradeClient::login(
        &username,
        &password,
        Environment::Sandbox,
    ).await?;

    let account = AccountNumber::new(&account_num);

    // Build a limit order to buy 1 share of SPY
    let order = NewOrderBuilder::new()
        .time_in_force(TimeInForce::Day)
        .order_type(OrderType::Limit)
        .price(dec!(400.00)) // Limit price
        .add_leg(OrderLeg::new(
            InstrumentType::Equity,
            "SPY",
            dec!(1),
            OrderAction::BuyToOpen,
        ))
        .build()?;

    println!("\nValidating order with dry run...");

    // Dry run to check if order would be accepted
    let dry_run = client.orders().dry_run(&account, order.clone()).await?;

    println!("Dry run result:");
    println!("  Buying Power Effect: {:?}", dry_run.buying_power_effect);
    println!("  Fee Calculation: {:?}", dry_run.fee_calculation);
    
    if dry_run.warnings.is_some() {
        println!("  Warnings: {:?}", dry_run.warnings);
    }

    // Uncomment to actually place the order:
    // println!("\nPlacing order...");
    // let response = client.orders().place(&account, order).await?;
    // println!("Order placed! ID: {:?}", response.order.id);
    // println!("Status: {:?}", response.order.status);

    println!("\n(Order not actually placed - uncomment code to place)");
    println!("Done!");
    Ok(())
}
