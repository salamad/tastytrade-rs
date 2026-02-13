//! Paginated stream for lazy iteration over API results.
//!
//! This module provides a [`PaginatedStream`] that implements the `Stream` trait,
//! allowing lazy, memory-efficient iteration over paginated API responses.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::Stream;
use serde::de::DeserializeOwned;

use super::ClientInner;
use crate::Result;

/// Default number of items per page.
pub const DEFAULT_PAGE_SIZE: i32 = 250;

/// Response structure for paginated endpoints.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PaginatedResponse<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Pagination metadata.
    pub pagination: Option<PaginationInfo>,
}

/// Pagination metadata from API response.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PaginationInfo {
    /// Items per page.
    pub per_page: i32,
    /// Current page offset (0-indexed).
    pub page_offset: i32,
    /// Total number of items across all pages.
    pub total_items: i32,
    /// Total number of pages.
    pub total_pages: i32,
}

impl PaginationInfo {
    /// Check if there are more pages after the current one.
    pub fn has_more(&self) -> bool {
        self.page_offset + 1 < self.total_pages
    }

    /// Get the next page offset, if available.
    pub fn next_page(&self) -> Option<i32> {
        if self.has_more() {
            Some(self.page_offset + 1)
        } else {
            None
        }
    }
}

/// Type alias for a boxed future used internally.
type BoxFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// A stream that lazily fetches pages from a paginated API endpoint.
///
/// This stream yields individual items from each page, automatically
/// fetching the next page when the current one is exhausted.
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
/// // Stream all transactions lazily
/// let mut stream = client.transactions().list_stream(&account, None);
///
/// while let Some(result) = stream.next().await {
///     let transaction = result?;
///     println!("{:?}", transaction);
/// }
/// # Ok(())
/// # }
/// ```
pub struct PaginatedStream<T> {
    /// Function to fetch a page by offset.
    fetch_page: Box<dyn Fn(i32) -> BoxFuture<'static, Result<PaginatedResponse<T>>> + Send + Sync>,
    /// Current page of items being yielded.
    current_items: Vec<T>,
    /// Next page offset to fetch, None if exhausted.
    next_page_offset: Option<i32>,
    /// Current in-flight fetch future.
    pending_fetch: Option<BoxFuture<'static, Result<PaginatedResponse<T>>>>,
}

impl<T> PaginatedStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    /// Create a new paginated stream.
    pub fn new<F>(fetch_page: F) -> Self
    where
        F: Fn(i32) -> BoxFuture<'static, Result<PaginatedResponse<T>>> + Send + Sync + 'static,
    {
        Self {
            fetch_page: Box::new(fetch_page),
            current_items: Vec::new(),
            next_page_offset: Some(0),
            pending_fetch: None,
        }
    }
}

impl<T> Stream for PaginatedStream<T>
where
    T: Unpin,
{
    type Item = Result<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        loop {
            // If we have items in the current page, yield the next one
            if !this.current_items.is_empty() {
                let item = this.current_items.remove(0);
                return Poll::Ready(Some(Ok(item)));
            }

            // Current page exhausted, check if we have a pending fetch
            if let Some(ref mut fut) = this.pending_fetch {
                match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(response)) => {
                        this.pending_fetch = None;
                        this.current_items = response.items;

                        // Update next page offset based on pagination info
                        this.next_page_offset = response
                            .pagination
                            .as_ref()
                            .and_then(|p| p.next_page());

                        // If we got items, continue the loop to yield them
                        if !this.current_items.is_empty() {
                            continue;
                        }

                        // No items and no more pages - we're done
                        return Poll::Ready(None);
                    }
                    Poll::Ready(Err(e)) => {
                        this.pending_fetch = None;
                        this.next_page_offset = None; // Stop on error
                        return Poll::Ready(Some(Err(e)));
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }

            // No pending fetch, check if we should start one
            if let Some(page_offset) = this.next_page_offset {
                let fut = (this.fetch_page)(page_offset);
                this.pending_fetch = Some(fut);
                // Loop back to poll the new future
                continue;
            }

            // No more pages to fetch
            return Poll::Ready(None);
        }
    }
}

impl<T> Unpin for PaginatedStream<T> {}

/// Builder for creating paginated streams with query parameters.
pub(crate) struct PaginatedStreamBuilder<T> {
    inner: Arc<ClientInner>,
    path: String,
    per_page: i32,
    _marker: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned + Unpin + Send + 'static> PaginatedStreamBuilder<T> {
    /// Create a new builder.
    pub(crate) fn new(inner: Arc<ClientInner>, path: impl Into<String>) -> Self {
        Self {
            inner,
            path: path.into(),
            per_page: DEFAULT_PAGE_SIZE,
            _marker: std::marker::PhantomData,
        }
    }

    /// Set the number of items per page.
    pub fn per_page(mut self, per_page: i32) -> Self {
        self.per_page = per_page;
        self
    }

    /// Build the stream with optional additional query parameters.
    pub fn build_with_query<Q>(self, query: Option<Q>) -> PaginatedStream<T>
    where
        Q: serde::Serialize + Clone + Send + Sync + 'static,
    {
        let inner = self.inner;
        let path = self.path;
        let per_page = self.per_page;

        PaginatedStream::new(move |page_offset: i32| {
            let inner = inner.clone();
            let path = path.clone();
            let query = query.clone();

            Box::pin(async move {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "kebab-case")]
                struct PaginationQuery<Q> {
                    per_page: i32,
                    page_offset: i32,
                    #[serde(flatten)]
                    extra: Option<Q>,
                }

                let pagination_query = PaginationQuery {
                    per_page,
                    page_offset,
                    extra: query,
                };

                inner
                    .get_with_query::<PaginatedResponse<T>, _>(&path, &pagination_query)
                    .await
            })
        })
    }

    /// Build the stream without additional query parameters.
    #[allow(dead_code)]
    pub fn build(self) -> PaginatedStream<T> {
        self.build_with_query::<()>(None)
    }
}
