//! Orders service for order placement and management.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::client::paginated::{PaginatedStream, PaginatedStreamBuilder, DEFAULT_PAGE_SIZE};
use crate::client::ClientInner;
use crate::models::{
    AccountNumber, DryRunResponse, NewComplexOrder, NewOrder, Order, OrderId,
    OrderStatus, PlacedOrderResponse,
};
use crate::Result;

/// Service for order operations.
///
/// # Example
///
/// ```no_run
/// use tastytrade_rs::AccountNumber;
/// use tastytrade_rs::models::{
///     NewOrderBuilder, OrderType, TimeInForce,
///     OrderLeg, InstrumentType, OrderAction, PriceEffect,
/// };
/// use rust_decimal_macros::dec;
///
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// let account = AccountNumber::new("5WV12345");
///
/// // Build an order
/// let order = NewOrderBuilder::new()
///     .time_in_force(TimeInForce::Day)
///     .order_type(OrderType::Limit)
///     .price(dec!(150.00))
///     .price_effect(PriceEffect::Debit)
///     .add_leg(OrderLeg::buy_equity("AAPL", dec!(10)))
///     .build()?;
///
/// // Dry run to validate
/// let dry_run = client.orders().dry_run(&account, order.clone()).await?;
/// println!("Fees: {:?}", dry_run.fee_calculation);
///
/// // Place the order
/// let result = client.orders().place(&account, order).await?;
/// println!("Order ID: {}", result.order.id);
/// # Ok(())
/// # }
/// ```
pub struct OrdersService {
    inner: Arc<ClientInner>,
}

/// Query parameters for listing orders.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrdersQuery {
    /// Filter by status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<OrderStatus>>,
    /// Filter by underlying symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_symbol: Option<String>,
    /// Filter orders after this time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<DateTime<Utc>>,
    /// Filter orders before this time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<DateTime<Utc>>,
    /// Number of results per page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
    /// Page offset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_offset: Option<i32>,
}

impl OrdersService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// List orders for an account with optional filters.
    pub async fn list(
        &self,
        account_number: &AccountNumber,
        query: Option<OrdersQuery>,
    ) -> Result<Vec<Order>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Order>,
        }

        let path = format!("/accounts/{}/orders", account_number);

        let response: Response = match query {
            Some(q) => self.inner.get_with_query(&path, &q).await?,
            None => self.inner.get(&path).await?,
        };

        Ok(response.items)
    }

    /// Get live (working) orders for an account.
    pub async fn live(&self, account_number: &AccountNumber) -> Result<Vec<Order>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Order>,
        }

        let response: Response = self
            .inner
            .get(&format!("/accounts/{}/orders/live", account_number))
            .await?;
        Ok(response.items)
    }

    /// Get a specific order by ID.
    pub async fn get(&self, account_number: &AccountNumber, order_id: &OrderId) -> Result<Order> {
        self.inner
            .get(&format!("/accounts/{}/orders/{}", account_number, order_id))
            .await
    }

    /// Place a new order.
    ///
    /// # Arguments
    ///
    /// * `account_number` - The account to place the order in
    /// * `order` - The order to place
    ///
    /// # Returns
    ///
    /// Returns the placed order along with buying power effects and fee calculations.
    pub async fn place(
        &self,
        account_number: &AccountNumber,
        order: NewOrder,
    ) -> Result<PlacedOrderResponse> {
        self.inner
            .post(&format!("/accounts/{}/orders", account_number), &order)
            .await
    }

    /// Dry-run an order to validate and see effects without placing it.
    ///
    /// Use this for order confirmation screens to show the user
    /// the expected fees and buying power impact.
    pub async fn dry_run(
        &self,
        account_number: &AccountNumber,
        order: NewOrder,
    ) -> Result<DryRunResponse> {
        self.inner
            .post(
                &format!("/accounts/{}/orders/dry-run", account_number),
                &order,
            )
            .await
    }

    /// Replace (modify) an existing order.
    ///
    /// The order must be in a cancellable/editable state.
    pub async fn replace(
        &self,
        account_number: &AccountNumber,
        order_id: &OrderId,
        order: NewOrder,
    ) -> Result<PlacedOrderResponse> {
        self.inner
            .put(
                &format!("/accounts/{}/orders/{}", account_number, order_id),
                &order,
            )
            .await
    }

    /// Cancel an order.
    ///
    /// The order must be in a cancellable state.
    pub async fn cancel(&self, account_number: &AccountNumber, order_id: &OrderId) -> Result<Order> {
        self.inner
            .delete(&format!("/accounts/{}/orders/{}", account_number, order_id))
            .await
    }

    /// Place a complex order (OCO/OTOCO).
    pub async fn place_complex(
        &self,
        account_number: &AccountNumber,
        order: NewComplexOrder,
    ) -> Result<PlacedOrderResponse> {
        self.inner
            .post(
                &format!("/accounts/{}/complex-orders", account_number),
                &order,
            )
            .await
    }

    /// Cancel all working orders for an account.
    ///
    /// Returns the list of cancelled orders.
    pub async fn cancel_all(&self, account_number: &AccountNumber) -> Result<Vec<Order>> {
        // First get all live orders
        let live_orders = self.live(account_number).await?;

        // Cancel each one
        let mut cancelled = Vec::new();
        for order in live_orders {
            if let Ok(cancelled_order) = self
                .cancel(account_number, &OrderId::new(&order.id))
                .await
            {
                cancelled.push(cancelled_order);
            }
        }

        Ok(cancelled)
    }

    /// Stream all orders for an account.
    ///
    /// This method returns a stream that lazily fetches pages of orders
    /// as you iterate. This is more memory-efficient than `list()` for large
    /// result sets (e.g., historical orders).
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
    /// // Stream all orders
    /// let mut stream = client.orders().list_stream(&account, None);
    ///
    /// while let Some(result) = stream.next().await {
    ///     let order = result?;
    ///     println!("Order {}: {:?}", order.id, order.status);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_stream(
        &self,
        account_number: &AccountNumber,
        query: Option<OrdersQueryStream>,
    ) -> PaginatedStream<Order> {
        let path = format!("/accounts/{}/orders", account_number);
        PaginatedStreamBuilder::<Order>::new(self.inner.clone(), path)
            .per_page(query.as_ref().and_then(|q| q.per_page).unwrap_or(DEFAULT_PAGE_SIZE))
            .build_with_query(query)
    }
}

/// Query parameters for streaming orders (without pagination fields).
///
/// Use this with `list_stream()`. Pagination is handled automatically by the stream.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct OrdersQueryStream {
    /// Filter by status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Vec<OrderStatus>>,
    /// Filter by underlying symbol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlying_symbol: Option<String>,
    /// Filter orders after this time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<DateTime<Utc>>,
    /// Filter orders before this time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_at: Option<DateTime<Utc>>,
    /// Results per page (controls fetch batch size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<i32>,
}
