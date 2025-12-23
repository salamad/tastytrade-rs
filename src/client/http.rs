//! HTTP client implementation for TastyTrade API.

use chrono::Duration;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use secrecy::ExposeSecret;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::api::{
    AccountsService, BalancesService, InstrumentsService, MarketDataService,
    MetricsService, OrdersService, SearchService, TransactionsService, WatchlistsService,
};
use crate::auth::Session;
use crate::{Environment, Error, Result};

use super::config::ClientConfig;

/// The main client for interacting with the TastyTrade API.
///
/// This client provides access to all API services through method calls
/// that return service structs. The client manages authentication,
/// request building, and response parsing.
///
/// # Example
///
/// ```no_run
/// use tastytrade_rs::{TastytradeClient, Environment, AccountNumber};
///
/// # async fn example() -> tastytrade_rs::Result<()> {
/// // Create a client with OAuth
/// let client = TastytradeClient::from_oauth(
///     "your-client-secret",
///     "your-refresh-token",
///     Environment::Sandbox,
/// ).await?;
///
/// // Use the accounts service
/// let accounts = client.accounts().list().await?;
///
/// // Use the orders service
/// if let Some(item) = accounts.first() {
///     let account_number = AccountNumber::new(&item.account.account_number);
///     let orders = client.orders().live(&account_number).await?;
/// }
/// # Ok(())
/// # }
/// ```
pub struct TastytradeClient {
    pub(crate) inner: Arc<ClientInner>,
}

pub(crate) struct ClientInner {
    pub(crate) http: reqwest::Client,
    pub(crate) session: Session,
    pub(crate) config: ClientConfig,
}

impl TastytradeClient {
    /// Create a new client with OAuth authentication.
    ///
    /// This is the recommended way to create a client for production use.
    pub async fn from_oauth(
        client_secret: impl Into<String>,
        refresh_token: impl Into<String>,
        env: Environment,
    ) -> Result<Self> {
        let session = Session::from_oauth(client_secret, refresh_token, env).await?;
        Self::with_session(session, ClientConfig::default())
    }

    /// Create a new client with username/password authentication.
    ///
    /// This method is provided for compatibility but OAuth is recommended.
    pub async fn from_credentials(
        username: impl Into<String>,
        password: impl Into<String>,
        env: Environment,
    ) -> Result<Self> {
        let session = Session::from_credentials(username, password, true, env).await?;
        Self::with_session(session, ClientConfig::default())
    }

    /// Login with username/password.
    ///
    /// This is an alias for [`from_credentials`](Self::from_credentials).
    pub async fn login(
        username: impl Into<String>,
        password: impl Into<String>,
        env: Environment,
    ) -> Result<Self> {
        Self::from_credentials(username, password, env).await
    }

    /// Login with username/password and custom configuration.
    pub async fn login_with_config(
        username: impl Into<String>,
        password: impl Into<String>,
        env: Environment,
        config: ClientConfig,
    ) -> Result<Self> {
        let session = Session::from_credentials(username, password, true, env).await?;
        Self::with_session(session, config)
    }

    /// Create a new client with an existing session and custom configuration.
    pub fn with_session(session: Session, config: ClientConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()?;

        Ok(Self {
            inner: Arc::new(ClientInner {
                http,
                session,
                config,
            }),
        })
    }

    /// Get the accounts service.
    pub fn accounts(&self) -> AccountsService {
        AccountsService::new(self.inner.clone())
    }

    /// Get the balances and positions service.
    pub fn balances(&self) -> BalancesService {
        BalancesService::new(self.inner.clone())
    }

    /// Get the orders service.
    pub fn orders(&self) -> OrdersService {
        OrdersService::new(self.inner.clone())
    }

    /// Get the instruments service.
    pub fn instruments(&self) -> InstrumentsService {
        InstrumentsService::new(self.inner.clone())
    }

    /// Get the market data service.
    pub fn market_data(&self) -> MarketDataService {
        MarketDataService::new(self.inner.clone())
    }

    /// Get the watchlists service.
    pub fn watchlists(&self) -> WatchlistsService {
        WatchlistsService::new(self.inner.clone())
    }

    /// Get the transactions service.
    pub fn transactions(&self) -> TransactionsService {
        TransactionsService::new(self.inner.clone())
    }

    /// Get the market metrics service.
    pub fn metrics(&self) -> MetricsService {
        MetricsService::new(self.inner.clone())
    }

    /// Get the symbol search service.
    pub fn search(&self) -> SearchService {
        SearchService::new(self.inner.clone())
    }

    /// Get the streaming services.
    #[cfg(feature = "streaming")]
    pub fn streaming(&self) -> crate::streaming::StreamingServices {
        crate::streaming::StreamingServices::new(self.inner.clone())
    }

    /// Manually refresh the session token.
    pub async fn refresh_session(&self) -> Result<()> {
        self.inner.session.refresh().await
    }

    /// Get the current environment.
    pub async fn environment(&self) -> Environment {
        self.inner.session.environment().await
    }

    /// Get a reference to the session.
    pub fn session(&self) -> &Session {
        &self.inner.session
    }
}

impl ClientInner {
    /// Get the base URL for API requests.
    pub(crate) async fn base_url(&self) -> String {
        self.session.environment().await.api_base_url().to_string()
    }

    /// Ensure the session is valid before making a request.
    pub(crate) async fn ensure_session_valid(&self) -> Result<()> {
        if self.config.auto_refresh_session {
            let buffer = Duration::seconds(self.config.refresh_buffer_secs);
            if self.session.expires_within(buffer).await {
                self.session.refresh().await?;
            }
        }
        Ok(())
    }

    /// Build request headers with authentication.
    pub(crate) async fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();

        let token = self.session.access_token().await;
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(token.expose_secret())
                .map_err(|_| Error::InvalidInput("Invalid token format".to_string()))?,
        );

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Add API version header if configured
        if let Some(ref version) = self.config.api_version {
            headers.insert(
                "Api-Version",
                HeaderValue::from_str(version.as_str())
                    .map_err(|_| Error::InvalidInput("Invalid API version".to_string()))?,
            );
        }

        Ok(headers)
    }

    /// Make a GET request.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.ensure_session_valid().await?;

        let url = format!("{}{}", self.base_url().await, path);
        let headers = self.build_headers().await?;

        let response = self.http.get(&url).headers(headers).send().await?;

        self.handle_response(response).await
    }

    /// Make a GET request with query parameters.
    pub(crate) async fn get_with_query<T: DeserializeOwned, Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
    ) -> Result<T> {
        self.ensure_session_valid().await?;

        let url = format!("{}{}", self.base_url().await, path);
        let headers = self.build_headers().await?;

        let response = self
            .http
            .get(&url)
            .headers(headers)
            .query(query)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a POST request.
    pub(crate) async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.ensure_session_valid().await?;

        let url = format!("{}{}", self.base_url().await, path);
        let headers = self.build_headers().await?;

        let response = self
            .http
            .post(&url)
            .headers(headers)
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a PUT request.
    pub(crate) async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.ensure_session_valid().await?;

        let url = format!("{}{}", self.base_url().await, path);
        let headers = self.build_headers().await?;

        let response = self
            .http
            .put(&url)
            .headers(headers)
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Make a DELETE request.
    pub(crate) async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.ensure_session_valid().await?;

        let url = format!("{}{}", self.base_url().await, path);
        let headers = self.build_headers().await?;

        let response = self.http.delete(&url).headers(headers).send().await?;

        self.handle_response(response).await
    }

    /// Handle an API response.
    async fn handle_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            let api_response: ApiResponse<T> = response.json().await?;
            Ok(api_response.data)
        } else {
            let status_code = status.as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();

            // Check for rate limiting
            if status_code == 429 {
                let retry_after = body
                    .get("retry-after")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60);
                return Err(Error::RateLimited {
                    retry_after_secs: retry_after,
                });
            }

            // Check for auth errors
            if status_code == 401 {
                return Err(Error::SessionExpired);
            }

            // Check for not found errors
            if status_code == 404 {
                let message = body
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Resource not found")
                    .to_string();
                return Err(Error::NotFound(message));
            }

            Err(Error::from_api_response(status_code, body))
        }
    }
}

/// Wrapper for API responses.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ApiResponse<T> {
    pub data: T,
    #[allow(dead_code)]
    pub context: Option<String>,
    #[allow(dead_code)]
    pub pagination: Option<Pagination>,
}

/// Pagination information for list responses.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Pagination {
    /// Items per page
    pub per_page: i32,
    /// Current page offset
    pub page_offset: i32,
    /// Total number of items
    pub total_items: i32,
    /// Total number of pages
    pub total_pages: i32,
}

impl Clone for TastytradeClient {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl std::fmt::Debug for TastytradeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TastytradeClient")
            .field("config", &self.inner.config)
            .finish()
    }
}
