//! Transactions service for account history.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::client::paginated::{PaginatedStream, PaginatedStreamBuilder, DEFAULT_PAGE_SIZE};
use crate::client::ClientInner;
use crate::models::{AccountNumber, Transaction, TransactionType};
use crate::Result;

/// Service for transaction history operations.
///
/// # Example
///
/// ```no_run
/// use tastytrade_rs::AccountNumber;
///
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// let account = AccountNumber::new("5WV12345");
///
/// // Get recent transactions
/// let transactions = client.transactions().list(&account, None).await?;
/// for txn in transactions {
///     println!("{:?}: {:?} - {:?}", txn.transaction_type, txn.symbol, txn.value);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TransactionsService {
    inner: Arc<ClientInner>,
}

/// Query parameters for listing transactions.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TransactionsQuery {
    /// Filter by transaction type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TransactionType>,
    /// Filter by symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Filter by underlying symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_symbol: Option<String>,
    /// Start of date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<DateTime<Utc>>,
    /// End of date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<DateTime<Utc>>,
    /// Results per page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
    /// Page offset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_offset: Option<i32>,
}

impl TransactionsService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// List transactions for an account.
    pub async fn list(
        &self,
        account_number: &AccountNumber,
        query: Option<TransactionsQuery>,
    ) -> Result<Vec<Transaction>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Transaction>,
        }

        let path = format!("/accounts/{}/transactions", account_number);

        let response: Response = match query {
            Some(q) => self.inner.get_with_query(&path, &q).await?,
            None => self.inner.get(&path).await?,
        };

        Ok(response.items)
    }

    /// Get a specific transaction by ID.
    pub async fn get(
        &self,
        account_number: &AccountNumber,
        transaction_id: &str,
    ) -> Result<Transaction> {
        self.inner
            .get(&format!(
                "/accounts/{}/transactions/{}",
                account_number, transaction_id
            ))
            .await
    }

    /// Get total fees for an account.
    pub async fn total_fees(
        &self,
        account_number: &AccountNumber,
    ) -> Result<TotalFees> {
        self.inner
            .get(&format!("/accounts/{}/transactions/total-fees", account_number))
            .await
    }

    /// Stream all transactions for an account.
    ///
    /// This method returns a stream that lazily fetches pages of transactions
    /// as you iterate. This is more memory-efficient than `list()` for large
    /// result sets.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use futures_util::StreamExt;
    /// use tastytrade_rs::AccountNumber;
    ///
    /// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
    /// let account = AccountNumber::new("5WV12345");
    ///
    /// // Stream all transactions
    /// let mut stream = client.transactions().list_stream(&account, None);
    ///
    /// while let Some(result) = stream.next().await {
    ///     let txn = result?;
    ///     println!("{:?}: {:?}", txn.transaction_type, txn.symbol);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_stream(
        &self,
        account_number: &AccountNumber,
        query: Option<TransactionsQueryStream>,
    ) -> PaginatedStream<Transaction> {
        let path = format!("/accounts/{}/transactions", account_number);
        PaginatedStreamBuilder::<Transaction>::new(self.inner.clone(), path)
            .per_page(query.as_ref().and_then(|q| q.per_page).unwrap_or(DEFAULT_PAGE_SIZE))
            .build_with_query(query)
    }
}

/// Query parameters for streaming transactions (without pagination fields).
///
/// Use this with `list_stream()`. Pagination is handled automatically by the stream.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TransactionsQueryStream {
    /// Filter by transaction type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<TransactionType>,
    /// Filter by symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Filter by underlying symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_symbol: Option<String>,
    /// Start of date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<DateTime<Utc>>,
    /// End of date range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<DateTime<Utc>>,
    /// Results per page (controls fetch batch size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
}

/// Total fees summary.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TotalFees {
    /// Total regulatory fees
    pub total_regulatory_fees: Option<rust_decimal::Decimal>,
    /// Total clearing fees
    pub total_clearing_fees: Option<rust_decimal::Decimal>,
    /// Total commissions
    pub total_commissions: Option<rust_decimal::Decimal>,
    /// Total other fees
    pub total_other_charges: Option<rust_decimal::Decimal>,
    /// Year to date total
    pub year_to_date_total: Option<rust_decimal::Decimal>,
}
