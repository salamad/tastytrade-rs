//! Market metrics service.

use std::sync::Arc;

use serde::Serialize;

use crate::client::ClientInner;
use crate::models::MarketMetric;
use crate::Result;

/// Service for market metrics operations.
///
/// Market metrics provide implied volatility rank/percentile, earnings dates,
/// dividend information, and other analytical data for underlyings.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// let metrics = client.metrics().get(&["AAPL", "SPY"]).await?;
/// for m in metrics {
///     println!("{}: IV Rank = {:?}", m.symbol, m.implied_volatility_index_rank);
/// }
/// # Ok(())
/// # }
/// ```
pub struct MetricsService {
    inner: Arc<ClientInner>,
}

impl MetricsService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get market metrics for symbols.
    pub async fn get(&self, symbols: &[&str]) -> Result<Vec<MarketMetric>> {
        #[derive(Serialize)]
        struct Query {
            symbols: String,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<MarketMetric>,
        }

        let query = Query {
            symbols: symbols.join(","),
        };

        let response: Response = self
            .inner
            .get_with_query("/market-metrics", &query)
            .await?;
        Ok(response.items)
    }

    /// Get market metrics for a single symbol.
    pub async fn get_one(&self, symbol: &str) -> Result<MarketMetric> {
        let metrics = self.get(&[symbol]).await?;
        metrics
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::InvalidSymbol(symbol.to_string()))
    }

    /// Get historical volatility data for a symbol.
    pub async fn historical_volatility(&self, symbol: &str) -> Result<Vec<VolatilityData>> {
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<VolatilityData>,
        }

        let response: Response = self
            .inner
            .get(&format!("/market-metrics/historic-corporate-events/{}/volatility", symbol))
            .await?;
        Ok(response.items)
    }
}

/// Historical volatility data point.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct VolatilityData {
    /// The date
    pub date: chrono::NaiveDate,
    /// Implied volatility
    pub implied_volatility: Option<rust_decimal::Decimal>,
    /// Historical volatility (realized)
    pub historical_volatility: Option<rust_decimal::Decimal>,
}
