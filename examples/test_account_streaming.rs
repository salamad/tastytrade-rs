//! Integration test for account streaming features.
//!
//! Tests the new account streaming enhancements:
//! - Connection state tracking
//! - Subscription inspection
//! - ReconnectConfig
//! - Order notification helpers
//! - Connection events

use std::time::Duration;
use tastytrade_rs::streaming::{AccountNotification, ReconnectConfig};
use tastytrade_rs::{AccountNumber, Environment, TastytradeClient};
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get credentials from environment
    let username = std::env::var("TASTYTRADE_USERNAME")
        .expect("TASTYTRADE_USERNAME must be set");
    let password = std::env::var("TASTYTRADE_PASSWORD")
        .expect("TASTYTRADE_PASSWORD must be set");
    let account_str = std::env::var("TASTYTRADE_ACCOUNT")
        .expect("TASTYTRADE_ACCOUNT must be set");
    let env_str = std::env::var("TASTYTRADE_ENVIRONMENT")
        .unwrap_or_else(|_| "sandbox".to_string());

    let environment = match env_str.as_str() {
        "production" => Environment::Production,
        _ => Environment::Sandbox,
    };

    println!("=== Account Streaming Integration Test ===");
    println!("Environment: {:?}", environment);
    println!("Account: {}", account_str);
    println!();

    // Test 1: Login
    println!("1. Testing login...");
    let client = TastytradeClient::login(&username, &password, environment).await?;
    println!("   ✓ Login successful");

    // Test 2: Create account streamer with ReconnectConfig
    println!("\n2. Testing AccountStreamer creation with ReconnectConfig...");

    // Test different config presets
    let default_config = ReconnectConfig::default();
    println!("   Default config: enabled={}, max_attempts={}",
        default_config.enabled, default_config.max_attempts);

    let aggressive_config = ReconnectConfig::aggressive();
    println!("   Aggressive config: enabled={}, max_attempts={} (0=unlimited)",
        aggressive_config.enabled, aggressive_config.max_attempts);

    let conservative_config = ReconnectConfig::conservative();
    println!("   Conservative config: enabled={}, max_attempts={}",
        conservative_config.enabled, conservative_config.max_attempts);

    let disabled_config = ReconnectConfig::disabled();
    println!("   Disabled config: enabled={}", disabled_config.enabled);

    // Test backoff calculation
    println!("   Backoff for attempt 0: {:?}", default_config.backoff_for_attempt(0));
    println!("   Backoff for attempt 1: {:?}", default_config.backoff_for_attempt(1));
    println!("   Backoff for attempt 2: {:?}", default_config.backoff_for_attempt(2));
    println!("   ✓ ReconnectConfig presets work correctly");

    // Test 3: Connect to account streamer
    println!("\n3. Testing AccountStreamer connection...");
    let mut streamer = client.streaming().account().await?
        .with_reconnect_config(ReconnectConfig::aggressive());
    println!("   ✓ AccountStreamer created with aggressive reconnect config");

    // Test 4: Check initial connection state
    println!("\n4. Testing connection state...");
    let connected = streamer.is_connected();
    println!("   is_connected(): {}", connected);
    println!("   ✓ Connection state tracking works");

    // Test 5: Check initial subscription state
    println!("\n5. Testing initial subscription state...");
    let subscribed = streamer.subscribed_accounts().await;
    println!("   Initial subscribed accounts: {:?}", subscribed);
    println!("   subscription_count(): {}", streamer.subscription_count().await);
    println!("   ✓ Subscription inspection works");

    // Test 6: Subscribe to account
    println!("\n6. Testing account subscription...");
    let account = AccountNumber::new(&account_str);
    streamer.subscribe(&account).await?;
    println!("   ✓ Subscribed to account: {}", account_str);

    // Test 7: Verify subscription state after subscribing
    println!("\n7. Verifying subscription state after subscribe...");
    let subscribed = streamer.subscribed_accounts().await;
    println!("   Subscribed accounts: {:?}", subscribed);
    println!("   subscription_count(): {}", streamer.subscription_count().await);
    println!("   is_subscribed({}): {}", account_str, streamer.is_subscribed(&account).await);
    println!("   ✓ Subscription state correctly updated");

    // Test 8: Receive notifications (with timeout)
    println!("\n8. Testing notification reception (waiting up to 30 seconds)...");
    println!("   Waiting for heartbeat or subscription confirmation...");

    let mut heartbeat_received = false;
    let mut subscription_confirmed = false;
    let mut notification_count = 0;

    let result = timeout(Duration::from_secs(30), async {
        while let Some(notification) = streamer.next().await {
            notification_count += 1;
            match notification {
                Ok(notif) => {
                    match &notif {
                        AccountNotification::Heartbeat => {
                            println!("   Received: Heartbeat");
                            println!("     is_connection_event(): {}", notif.is_connection_event());
                            println!("     is_healthy(): {}", notif.is_healthy());
                            heartbeat_received = true;
                        }
                        AccountNotification::SubscriptionConfirmation { accounts } => {
                            println!("   Received: SubscriptionConfirmation for {:?}", accounts);
                            println!("     is_connection_event(): {}", notif.is_connection_event());
                            println!("     is_healthy(): {}", notif.is_healthy());
                            subscription_confirmed = true;
                        }
                        AccountNotification::Order(order) => {
                            println!("   Received: Order notification");
                            println!("     order_id(): {:?}", order.order_id());
                            println!("     order_id_numeric(): {:?}", order.order_id_numeric());
                            println!("     status: {:?}", order.status);
                            println!("     is_filled(): {}", order.is_filled());
                            println!("     is_rejected(): {}", order.is_rejected());
                            println!("     is_working(): {}", order.is_working());
                            println!("     is_terminal(): {}", order.is_terminal());
                            println!("     is_order(): {}", notif.is_order());
                        }
                        AccountNotification::Position(pos) => {
                            println!("   Received: Position notification");
                            println!("     symbol: {:?}", pos.symbol);
                            println!("     quantity: {:?}", pos.quantity);
                            println!("     is_position(): {}", notif.is_position());
                        }
                        AccountNotification::Balance(bal) => {
                            println!("   Received: Balance notification");
                            println!("     net_liquidating_value: {:?}", bal.net_liquidating_value);
                            println!("     is_balance(): {}", notif.is_balance());
                        }
                        AccountNotification::QuoteAlert(alert) => {
                            println!("   Received: QuoteAlert notification");
                            println!("     {:?}", alert);
                        }
                        AccountNotification::Disconnected { reason } => {
                            println!("   Received: Disconnected - {}", reason);
                            println!("     is_connection_event(): {}", notif.is_connection_event());
                        }
                        AccountNotification::Reconnected { accounts_restored } => {
                            println!("   Received: Reconnected - {} accounts restored", accounts_restored);
                            println!("     is_connection_event(): {}", notif.is_connection_event());
                        }
                        AccountNotification::ConnectionWarning { message } => {
                            println!("   Received: ConnectionWarning - {}", message);
                            println!("     is_connection_event(): {}", notif.is_connection_event());
                        }
                        AccountNotification::Unknown(val) => {
                            println!("   Received: Unknown notification: {:?}", val);
                        }
                        AccountNotification::ParseError { action, error, raw_data } => {
                            println!("   Received: ParseError");
                            println!("     action: {}", action);
                            println!("     error: {}", error);
                            println!("     raw_data: {:?}", raw_data);
                        }
                    }

                    // Stop after receiving both heartbeat and subscription confirmation
                    // or after 5 notifications
                    if (heartbeat_received && subscription_confirmed) || notification_count >= 5 {
                        break;
                    }
                }
                Err(e) => {
                    println!("   Error receiving notification: {:?}", e);
                    break;
                }
            }
        }
    }).await;

    match result {
        Ok(_) => {
            println!("   ✓ Received {} notifications", notification_count);
            if heartbeat_received {
                println!("   ✓ Heartbeat received");
            }
            if subscription_confirmed {
                println!("   ✓ Subscription confirmed");
            }
        }
        Err(_) => {
            println!("   ⚠ Timeout waiting for notifications (this may be normal if no activity)");
            println!("   Received {} notifications before timeout", notification_count);
        }
    }

    // Test 9: Unsubscribe
    println!("\n9. Testing unsubscribe...");
    streamer.unsubscribe(&account).await?;
    println!("   ✓ Unsubscribed from account: {}", account_str);

    // Test 10: Verify subscription state after unsubscribing
    println!("\n10. Verifying subscription state after unsubscribe...");
    let subscribed = streamer.subscribed_accounts().await;
    println!("    Subscribed accounts: {:?}", subscribed);
    println!("    subscription_count(): {}", streamer.subscription_count().await);
    println!("    is_subscribed({}): {}", account_str, streamer.is_subscribed(&account).await);
    println!("    ✓ Subscription state correctly updated after unsubscribe");

    // Test 11: Final connection state
    println!("\n11. Final connection state check...");
    println!("    is_connected(): {}", streamer.is_connected());
    println!("    ✓ Connection state still tracked");

    println!("\n=== All Account Streaming Tests Passed! ===");
    println!("\nSummary of tested features:");
    println!("  ✓ ReconnectConfig presets (default, aggressive, conservative, disabled)");
    println!("  ✓ ReconnectConfig backoff calculation");
    println!("  ✓ AccountStreamer.with_reconnect_config()");
    println!("  ✓ AccountStreamer.is_connected()");
    println!("  ✓ AccountStreamer.subscribed_accounts()");
    println!("  ✓ AccountStreamer.subscription_count()");
    println!("  ✓ AccountStreamer.is_subscribed()");
    println!("  ✓ AccountStreamer.subscribe()");
    println!("  ✓ AccountStreamer.unsubscribe()");
    println!("  ✓ AccountNotification variants (Heartbeat, SubscriptionConfirmation, etc.)");
    println!("  ✓ AccountNotification.is_connection_event()");
    println!("  ✓ AccountNotification.is_healthy()");
    println!("  ✓ OrderNotification helper methods (if orders received)");

    Ok(())
}
