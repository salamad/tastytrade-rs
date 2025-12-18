//! Watchlists service.

use std::sync::Arc;

use crate::client::ClientInner;
use crate::models::{PublicWatchlist, Watchlist, WatchlistEntry};
use crate::Result;

/// Service for watchlist operations.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Get public watchlists (curated by tastytrade)
/// let public = client.watchlists().public().await?;
///
/// // Get user's watchlists
/// let my_lists = client.watchlists().list().await?;
/// # Ok(())
/// # }
/// ```
pub struct WatchlistsService {
    inner: Arc<ClientInner>,
}

impl WatchlistsService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get public watchlists (curated by tastytrade).
    pub async fn public(&self) -> Result<Vec<PublicWatchlist>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<PublicWatchlist>,
        }

        let response: Response = self.inner.get("/public-watchlists").await?;
        Ok(response.items)
    }

    /// Get the user's watchlists.
    pub async fn list(&self) -> Result<Vec<Watchlist>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Watchlist>,
        }

        let response: Response = self.inner.get("/watchlists").await?;
        Ok(response.items)
    }

    /// Get a specific watchlist by name.
    pub async fn get(&self, name: &str) -> Result<Watchlist> {
        self.inner.get(&format!("/watchlists/{}", name)).await
    }

    /// Create a new watchlist.
    pub async fn create(&self, name: &str, entries: Vec<WatchlistEntry>) -> Result<Watchlist> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Request {
            name: String,
            watchlist_entries: Vec<WatchlistEntry>,
        }

        self.inner
            .post(
                "/watchlists",
                &Request {
                    name: name.to_string(),
                    watchlist_entries: entries,
                },
            )
            .await
    }

    /// Update a watchlist.
    pub async fn update(&self, name: &str, entries: Vec<WatchlistEntry>) -> Result<Watchlist> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Request {
            watchlist_entries: Vec<WatchlistEntry>,
        }

        self.inner
            .put(
                &format!("/watchlists/{}", name),
                &Request {
                    watchlist_entries: entries,
                },
            )
            .await
    }

    /// Delete a watchlist.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let _: serde_json::Value = self.inner.delete(&format!("/watchlists/{}", name)).await?;
        Ok(())
    }
}
