//! Accounts service for customer and account operations.

use std::sync::Arc;

use crate::client::ClientInner;
use crate::models::{Account, AccountItem, AccountNumber, Customer};
use crate::Result;

/// Service for account-related operations.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Get customer info
/// let customer = client.accounts().me().await?;
/// println!("Hello, {}!", customer.username);
///
/// // List accounts
/// let accounts = client.accounts().list().await?;
/// for account in accounts {
///     println!("Account: {}", account.account.account_number);
/// }
/// # Ok(())
/// # }
/// ```
pub struct AccountsService {
    inner: Arc<ClientInner>,
}

impl AccountsService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get the current customer's profile.
    ///
    /// Returns information about the authenticated user.
    pub async fn me(&self) -> Result<Customer> {
        self.inner.get("/customers/me").await
    }

    /// List all accounts for the current customer.
    ///
    /// Returns a list of all trading accounts the user has access to.
    pub async fn list(&self) -> Result<Vec<AccountItem>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<AccountItem>,
        }
        let response: Response = self.inner.get("/customers/me/accounts").await?;
        Ok(response.items)
    }

    /// Get details for a specific account.
    ///
    /// # Arguments
    ///
    /// * `account_number` - The account number to retrieve
    pub async fn get(&self, account_number: &AccountNumber) -> Result<Account> {
        self.inner
            .get(&format!("/customers/me/accounts/{}", account_number))
            .await
    }
}
