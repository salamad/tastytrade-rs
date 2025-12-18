//! Symbol search service.

use std::sync::Arc;

use crate::client::ClientInner;
use crate::models::SymbolSearchResult;
use crate::Result;

/// Service for symbol search operations.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Search for symbols
/// let results = client.search().search("AAPL").await?;
/// for result in results {
///     println!("{}: {:?}", result.symbol, result.description);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SearchService {
    inner: Arc<ClientInner>,
}

impl SearchService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Search for symbols matching a query.
    ///
    /// The search matches against symbol names and descriptions.
    pub async fn search(&self, query: &str) -> Result<Vec<SymbolSearchResult>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<SymbolSearchResult>,
        }

        let response: Response = self
            .inner
            .get(&format!("/symbols/search/{}", query))
            .await?;
        Ok(response.items)
    }
}
