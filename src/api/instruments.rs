//! Instruments service for retrieving instrument data.

use std::sync::Arc;

use serde::Serialize;

use crate::client::ClientInner;
use crate::models::{
    Cryptocurrency, Equity, EquityOption, Future, FutureOption, NestedOptionChain,
};
use crate::Result;

/// Service for instrument data operations.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Get equity info
/// let equities = client.instruments().equities(&["AAPL", "TSLA"]).await?;
///
/// // Get option chain
/// let chain = client.instruments().option_chain("AAPL").await?;
/// # Ok(())
/// # }
/// ```
pub struct InstrumentsService {
    inner: Arc<ClientInner>,
}

impl InstrumentsService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get equity instruments by symbols.
    pub async fn equities(&self, symbols: &[&str]) -> Result<Vec<Equity>> {
        #[derive(Serialize)]
        struct Query<'a> {
            symbol: &'a [&'a str],
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Equity>,
        }

        let response: Response = self
            .inner
            .get_with_query("/instruments/equities", &Query { symbol: symbols })
            .await?;
        Ok(response.items)
    }

    /// Get a single equity by symbol.
    pub async fn equity(&self, symbol: &str) -> Result<Equity> {
        self.inner
            .get(&format!("/instruments/equities/{}", symbol))
            .await
    }

    /// Get equity options by symbols.
    pub async fn equity_options(&self, symbols: &[&str]) -> Result<Vec<EquityOption>> {
        #[derive(Serialize)]
        struct Query<'a> {
            symbol: &'a [&'a str],
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<EquityOption>,
        }

        let response: Response = self
            .inner
            .get_with_query("/instruments/equity-options", &Query { symbol: symbols })
            .await?;
        Ok(response.items)
    }

    /// Get a nested option chain for an underlying symbol.
    ///
    /// Returns all expirations and strikes organized hierarchically.
    pub async fn option_chain(&self, underlying: &str) -> Result<NestedOptionChain> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<NestedOptionChain>,
        }

        let response: Response = self
            .inner
            .get(&format!("/option-chains/{}/nested", underlying))
            .await?;

        response
            .items
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::InvalidSymbol(underlying.to_string()))
    }

    /// Get futures contracts.
    pub async fn futures(&self, symbols: Option<&[&str]>) -> Result<Vec<Future>> {
        #[derive(Serialize)]
        struct Query<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            symbol: Option<&'a [&'a str]>,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Future>,
        }

        let response: Response = self
            .inner
            .get_with_query("/instruments/futures", &Query { symbol: symbols })
            .await?;
        Ok(response.items)
    }

    /// Get a nested futures option chain.
    pub async fn futures_option_chain(&self, product_code: &str) -> Result<NestedOptionChain> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<NestedOptionChain>,
        }

        let response: Response = self
            .inner
            .get(&format!("/futures-option-chains/{}/nested", product_code))
            .await?;

        response
            .items
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::InvalidSymbol(product_code.to_string()))
    }

    /// Get futures options.
    pub async fn futures_options(&self, symbols: &[&str]) -> Result<Vec<FutureOption>> {
        #[derive(Serialize)]
        struct Query<'a> {
            symbol: &'a [&'a str],
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<FutureOption>,
        }

        let response: Response = self
            .inner
            .get_with_query("/instruments/future-options", &Query { symbol: symbols })
            .await?;
        Ok(response.items)
    }

    /// Get cryptocurrencies.
    pub async fn cryptocurrencies(&self) -> Result<Vec<Cryptocurrency>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Cryptocurrency>,
        }

        let response: Response = self.inner.get("/instruments/cryptocurrencies").await?;
        Ok(response.items)
    }

    /// Get a single cryptocurrency by symbol.
    pub async fn cryptocurrency(&self, symbol: &str) -> Result<Cryptocurrency> {
        self.inner
            .get(&format!("/instruments/cryptocurrencies/{}", symbol))
            .await
    }
}
