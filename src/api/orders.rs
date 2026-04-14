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
/// println!("Order ID: {:?}", result.order.id);
/// # Ok(())
/// # }
/// ```
pub struct OrdersService {
    inner: Arc<ClientInner>,
}

/// Query parameters for listing orders.
#[derive(Debug, Default, Clone)]
pub struct OrdersQuery {
    /// Filter by status
    pub status: Option<Vec<OrderStatus>>,
    /// Filter by underlying symbol
    pub underlying_symbol: Option<String>,
    /// Filter orders after this time
    pub start_at: Option<DateTime<Utc>>,
    /// Filter orders before this time
    pub end_at: Option<DateTime<Utc>>,
    /// Number of results per page
    pub per_page: Option<i32>,
    /// Page offset
    pub page_offset: Option<i32>,
}

impl OrdersQuery {
    /// Build a query string from the query parameters.
    fn to_query_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(statuses) = &self.status {
            for status in statuses {
                let status_str = match status {
                    OrderStatus::Received => "Received",
                    OrderStatus::Routed => "Routed",
                    OrderStatus::InFlight => "In Flight",
                    OrderStatus::Live => "Live",
                    OrderStatus::CancelRequested => "Cancel Requested",
                    OrderStatus::ReplaceRequested => "Replace Requested",
                    OrderStatus::Contingent => "Contingent",
                    OrderStatus::Filled => "Filled",
                    OrderStatus::Cancelled => "Cancelled",
                    OrderStatus::Expired => "Expired",
                    OrderStatus::Rejected => "Rejected",
                    OrderStatus::Removed => "Removed",
                    OrderStatus::PartiallyRemoved => "Partially Removed",
                };
                parts.push(format!("status[]={}", urlencoding::encode(status_str)));
            }
        }

        if let Some(symbol) = &self.underlying_symbol {
            parts.push(format!("underlying-symbol={}", urlencoding::encode(symbol)));
        }

        if let Some(start) = &self.start_at {
            parts.push(format!("start-at={}", start.to_rfc3339()));
        }

        if let Some(end) = &self.end_at {
            parts.push(format!("end-at={}", end.to_rfc3339()));
        }

        if let Some(per_page) = self.per_page {
            parts.push(format!("per-page={}", per_page));
        }

        if let Some(page_offset) = self.page_offset {
            parts.push(format!("page-offset={}", page_offset));
        }

        parts.join("&")
    }
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

        let base_path = format!("/accounts/{}/orders", account_number);
        let path = match query {
            Some(q) => {
                let query_string = q.to_query_string();
                if query_string.is_empty() {
                    base_path
                } else {
                    format!("{}?{}", base_path, query_string)
                }
            }
            None => base_path,
        };

        let response: Response = self.inner.get(&path).await?;
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
    ///
    /// TastyTrade returns HTTP 422 with `preflight_check_failure` when one or
    /// more validation checks fail. The response body still contains a valid
    /// `DryRunResponse` with `errors` and `warnings` arrays populated. This
    /// method parses the 422 body as a `DryRunResponse` so the caller can
    /// inspect the specific failures rather than getting a generic error.
    pub async fn dry_run(
        &self,
        account_number: &AccountNumber,
        order: NewOrder,
    ) -> Result<DryRunResponse> {
        self.inner.ensure_session_valid().await?;

        let url = format!(
            "{}/accounts/{}/orders/dry-run",
            self.inner.base_url().await,
            account_number
        );
        let headers = self.inner.build_headers().await?;

        let response = self
            .inner
            .http
            .post(&url)
            .headers(headers)
            .json(&order)
            .send()
            .await?;

        let status = response.status();

        if status.is_success() {
            // 200: standard ApiResponse wrapper with DryRunResponse in data
            let body = response.text().await?;
            let api_response: crate::client::ApiResponse<DryRunResponse> =
                serde_json::from_str(&body).map_err(|e| {
                    tracing::error!(
                        error = %e,
                        body_preview = &body[..body.len().min(500)],
                        "Failed to deserialize dry-run response"
                    );
                    crate::Error::Json(e)
                })?;
            Ok(api_response.data)
        } else if status.as_u16() == 422 {
            // 422 (preflight_check_failure): TastyTrade returns the error details
            // in a different structure than success responses. Extract warnings
            // and errors from the response body.
            let body = response.text().await?;
            let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

            tracing::debug!(
                body_preview = &body[..body.len().min(1000)],
                "Dry-run 422 response body"
            );

            // Try to parse as ApiResponse<DryRunResponse> first (some endpoints
            // wrap 422 in the standard format)
            if let Ok(api_response) =
                serde_json::from_str::<crate::client::ApiResponse<DryRunResponse>>(&body)
            {
                return Ok(api_response.data);
            }

            // Try to parse as bare DryRunResponse (no data wrapper)
            if let Ok(dry_run) = serde_json::from_str::<DryRunResponse>(&body) {
                return Ok(dry_run);
            }

            // Fall back: extract errors from the error object and build a
            // DryRunResponse with just the error information
            let mut errors = Vec::new();

            // Check for top-level error.errors array
            if let Some(error_obj) = json.get("error") {
                if let Some(error_array) = error_obj.get("errors").and_then(|e| e.as_array()) {
                    for err in error_array {
                        let code = err
                            .get("code")
                            .and_then(|c| c.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let message = err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown preflight error")
                            .to_string();
                        errors.push(crate::models::OrderMessage { code, message, preflight_id: None });
                    }
                }
                // If no errors array, use the top-level error message
                if errors.is_empty() {
                    let message = error_obj
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Preflight check failed")
                        .to_string();
                    let code = error_obj
                        .get("code")
                        .and_then(|c| c.as_str())
                        .unwrap_or("preflight_check_failure")
                        .to_string();
                    errors.push(crate::models::OrderMessage { code, message, preflight_id: None });
                }
            }

            // Extract warnings similarly
            let mut warnings = Vec::new();
            if let Some(warning_array) = json
                .get("error")
                .and_then(|e| e.get("warnings"))
                .and_then(|w| w.as_array())
            {
                for warn in warning_array {
                    let code = warn
                        .get("code")
                        .and_then(|c| c.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let message = warn
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown warning")
                        .to_string();
                    warnings.push(crate::models::OrderMessage { code, message, preflight_id: None });
                }
            }

            Ok(DryRunResponse {
                order: None,
                buying_power_effect: None,
                fee_calculation: None,
                warnings: if warnings.is_empty() {
                    None
                } else {
                    Some(warnings)
                },
                errors: if errors.is_empty() {
                    None
                } else {
                    Some(errors)
                },
            })
        } else {
            // Non-422 errors (401, 429, 500, etc.) — use standard error handling
            let status_code = status.as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();

            if status_code == 401 {
                return Err(crate::Error::SessionExpired);
            }

            Err(crate::Error::from_api_response(status_code, body))
        }
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
            if let Some(id) = &order.id {
                if let Ok(cancelled_order) = self
                    .cancel(account_number, &OrderId::new(id))
                    .await
                {
                    cancelled.push(cancelled_order);
                }
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
    ///     println!("Order {:?}: {:?}", order.id, order.status);
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
