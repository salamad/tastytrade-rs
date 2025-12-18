//! Session management for TastyTrade API authentication.

use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{Environment, Error, Result};

/// Authentication session for the TastyTrade API.
///
/// The session manages authentication tokens and handles automatic
/// refresh of expired tokens.
///
/// # Thread Safety
///
/// `Session` is designed to be shared across multiple tasks. It uses
/// internal locking to safely manage token refresh.
#[derive(Clone)]
pub struct Session {
    inner: Arc<RwLock<SessionInner>>,
}

struct SessionInner {
    env: Environment,
    access_token: SecretString,
    expires_at: DateTime<Utc>,
    refresh_token: Option<SecretString>,
    client_secret: Option<SecretString>,
    remember_token: Option<SecretString>,
    user: Option<User>,
}

impl Session {
    /// Create a session using OAuth credentials.
    ///
    /// This is the recommended authentication method. The refresh token
    /// is long-lived (effectively non-expiring), while access tokens
    /// are short-lived (~15 minutes).
    ///
    /// # Arguments
    ///
    /// * `client_secret` - Your OAuth client secret
    /// * `refresh_token` - Your OAuth refresh token
    /// * `env` - The API environment to use
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tastytrade_rs::{Session, Environment};
    ///
    /// # async fn example() -> tastytrade_rs::Result<()> {
    /// let session = Session::from_oauth(
    ///     std::env::var("TASTYTRADE_CLIENT_SECRET").unwrap(),
    ///     std::env::var("TASTYTRADE_REFRESH_TOKEN").unwrap(),
    ///     Environment::Sandbox,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_oauth(
        client_secret: impl Into<String>,
        refresh_token: impl Into<String>,
        env: Environment,
    ) -> Result<Self> {
        let client_secret = SecretString::from(client_secret.into());
        let refresh_token = SecretString::from(refresh_token.into());

        // Perform initial token exchange
        let token_response =
            Self::exchange_token(&client_secret, &refresh_token, env).await?;

        Ok(Self {
            inner: Arc::new(RwLock::new(SessionInner {
                env,
                access_token: SecretString::from(token_response.access_token),
                expires_at: token_response.expires_at,
                refresh_token: Some(refresh_token),
                client_secret: Some(client_secret),
                remember_token: None,
                user: None,
            })),
        })
    }

    /// Create a session using username and password.
    ///
    /// This is the legacy authentication method, supported for
    /// compatibility. Consider using OAuth for new applications.
    ///
    /// # Arguments
    ///
    /// * `username` - Your TastyTrade username or email
    /// * `password` - Your TastyTrade password
    /// * `remember_me` - Whether to request a remember token
    /// * `env` - The API environment to use
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tastytrade_rs::{Session, Environment};
    ///
    /// # async fn example() -> tastytrade_rs::Result<()> {
    /// let session = Session::from_credentials(
    ///     "your-username",
    ///     "your-password",
    ///     true,
    ///     Environment::Sandbox,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_credentials(
        username: impl Into<String>,
        password: impl Into<String>,
        remember_me: bool,
        env: Environment,
    ) -> Result<Self> {
        let login_response =
            Self::login(&username.into(), &password.into(), remember_me, env).await?;

        Ok(Self {
            inner: Arc::new(RwLock::new(SessionInner {
                env,
                access_token: SecretString::from(login_response.session_token),
                expires_at: login_response.session_expiration,
                refresh_token: None,
                client_secret: None,
                remember_token: login_response
                    .remember_token
                    .map(SecretString::from),
                user: Some(login_response.user),
            })),
        })
    }

    /// Create a session using a remember token.
    ///
    /// Remember tokens are obtained from a previous login with
    /// `remember_me: true`.
    pub async fn from_remember_token(
        username: impl Into<String>,
        remember_token: impl Into<String>,
        env: Environment,
    ) -> Result<Self> {
        let login_response =
            Self::login_with_remember_token(&username.into(), &remember_token.into(), env)
                .await?;

        Ok(Self {
            inner: Arc::new(RwLock::new(SessionInner {
                env,
                access_token: SecretString::from(login_response.session_token),
                expires_at: login_response.session_expiration,
                refresh_token: None,
                client_secret: None,
                remember_token: login_response
                    .remember_token
                    .map(SecretString::from),
                user: Some(login_response.user),
            })),
        })
    }

    /// Check if the session token has expired.
    pub async fn is_expired(&self) -> bool {
        let inner = self.inner.read().await;
        Utc::now() >= inner.expires_at
    }

    /// Check if the session will expire within the given buffer period.
    pub async fn expires_within(&self, buffer: Duration) -> bool {
        let inner = self.inner.read().await;
        Utc::now() + buffer >= inner.expires_at
    }

    /// Get the session expiration time.
    pub async fn expires_at(&self) -> DateTime<Utc> {
        self.inner.read().await.expires_at
    }

    /// Refresh the access token.
    ///
    /// For OAuth sessions, this exchanges the refresh token for a new
    /// access token. For legacy sessions, this is a no-op unless a
    /// remember token is available.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session doesn't have refresh credentials
    /// - The API returns an error during refresh
    pub async fn refresh(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        // Try OAuth refresh first
        if let (Some(client_secret), Some(refresh_token)) =
            (&inner.client_secret, &inner.refresh_token)
        {
            let token_response =
                Self::exchange_token(client_secret, refresh_token, inner.env).await?;
            inner.access_token = SecretString::from(token_response.access_token);
            inner.expires_at = token_response.expires_at;
            return Ok(());
        }

        // Fall back to remember token if available
        if let (Some(remember_token), Some(user)) = (&inner.remember_token, &inner.user) {
            let login_response = Self::login_with_remember_token(
                &user.username,
                remember_token.expose_secret(),
                inner.env,
            )
            .await?;
            inner.access_token = SecretString::from(login_response.session_token);
            inner.expires_at = login_response.session_expiration;
            if let Some(new_token) = login_response.remember_token {
                inner.remember_token = Some(SecretString::from(new_token));
            }
            return Ok(());
        }

        Err(Error::SessionExpired)
    }

    /// Get the current access token.
    ///
    /// This method does not check if the token is expired. Use
    /// `ensure_valid()` to refresh if needed first.
    pub(crate) async fn access_token(&self) -> SecretString {
        self.inner.read().await.access_token.clone()
    }

    /// Ensure the session is valid, refreshing if necessary.
    ///
    /// This method checks if the token is about to expire (within 60 seconds)
    /// and refreshes it if needed.
    pub async fn ensure_valid(&self) -> Result<()> {
        if self.expires_within(Duration::seconds(60)).await {
            self.refresh().await?;
        }
        Ok(())
    }

    /// Get the environment this session is connected to.
    pub async fn environment(&self) -> Environment {
        self.inner.read().await.env
    }

    /// Get the current user (if available).
    pub async fn user(&self) -> Option<User> {
        self.inner.read().await.user.clone()
    }

    /// Get the remember token (if available).
    ///
    /// Store this securely to create future sessions without re-entering
    /// credentials.
    pub async fn remember_token(&self) -> Option<String> {
        self.inner
            .read()
            .await
            .remember_token
            .as_ref()
            .map(|t| t.expose_secret().to_string())
    }

    /// Destroy the session (logout).
    pub async fn destroy(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let client = reqwest::Client::new();
        let url = format!("{}/sessions", inner.env.api_base_url());

        let response = client
            .delete(&url)
            .header("Authorization", inner.access_token.expose_secret())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            return Err(Error::from_api_response(status, body));
        }

        Ok(())
    }

    // Private helper methods

    async fn exchange_token(
        client_secret: &SecretString,
        refresh_token: &SecretString,
        env: Environment,
    ) -> Result<TokenResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/oauth/token", env.api_base_url());

        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "grant_type": "refresh_token",
                "client_secret": client_secret.expose_secret(),
                "refresh_token": refresh_token.expose_secret(),
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            return Err(Error::Authentication(format!(
                "Token exchange failed ({}): {:?}",
                status, body
            )));
        }

        let mut token_response: TokenResponse = response.json().await?;
        token_response.expires_at =
            Utc::now() + Duration::seconds(token_response.expires_in);
        Ok(token_response)
    }

    async fn login(
        username: &str,
        password: &str,
        remember_me: bool,
        env: Environment,
    ) -> Result<LoginResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/sessions", env.api_base_url());

        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "login": username,
                "password": password,
                "remember-me": remember_me,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            return Err(Error::Authentication(format!(
                "Login failed ({}): {:?}",
                status, body
            )));
        }

        let api_response: ApiResponse<LoginResponse> = response.json().await?;
        Ok(api_response.data)
    }

    async fn login_with_remember_token(
        username: &str,
        remember_token: &str,
        env: Environment,
    ) -> Result<LoginResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/sessions", env.api_base_url());

        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "login": username,
                "remember-token": remember_token,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            return Err(Error::Authentication(format!(
                "Remember token login failed ({}): {:?}",
                status, body
            )));
        }

        let api_response: ApiResponse<LoginResponse> = response.json().await?;
        Ok(api_response.data)
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("env", &"...")
            .field("access_token", &"[REDACTED]")
            .field("expires_at", &"...")
            .finish()
    }
}

/// User information from login response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct User {
    /// User's email address
    pub email: String,
    /// Username
    pub username: String,
    /// External ID
    #[serde(default)]
    pub external_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: i64,
    #[serde(skip)]
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LoginResponse {
    session_token: String,
    session_expiration: DateTime<Utc>,
    remember_token: Option<String>,
    user: User,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: T,
    #[allow(dead_code)]
    context: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_debug_redacts_token() {
        // Ensure we don't leak tokens in debug output
        let debug_str = format!("{:?}", Session {
            inner: Arc::new(RwLock::new(SessionInner {
                env: Environment::Sandbox,
                access_token: SecretString::from("super-secret-token".to_string()),
                expires_at: Utc::now(),
                refresh_token: None,
                client_secret: None,
                remember_token: None,
                user: None,
            })),
        });

        assert!(!debug_str.contains("super-secret-token"));
        assert!(debug_str.contains("REDACTED"));
    }
}
