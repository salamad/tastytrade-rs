//! Market data service for snapshot quotes.

use std::sync::Arc;

use serde::Serialize;

use crate::client::ClientInner;
use crate::models::{DetailedQuote, InstrumentType, MarketData};
use crate::{Error, Result};

/// Maximum number of symbols per request.
pub const MAX_SYMBOLS_PER_REQUEST: usize = 100;

/// Service for market data operations.
///
/// Note: This provides snapshot (one-time) market data. For streaming
/// real-time data, use the DXLink streamer via `client.streaming().dxlink()`.
///
/// # Example
///
/// ```no_run
/// use tastytrade_rs::models::InstrumentType;
///
/// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
/// // Get quote for a single symbol
/// let quote = client.market_data().get("AAPL", InstrumentType::Equity).await?;
/// println!("AAPL: bid={:?}, ask={:?}", quote.bid, quote.ask);
///
/// // Get quotes for multiple symbols
/// let quotes = client.market_data().get_many(&["AAPL", "TSLA"], InstrumentType::Equity).await?;
/// # Ok(())
/// # }
/// ```
pub struct MarketDataService {
    inner: Arc<ClientInner>,
}

impl MarketDataService {
    pub(crate) fn new(inner: Arc<ClientInner>) -> Self {
        Self { inner }
    }

    /// Get market data for a single symbol.
    pub async fn get(&self, symbol: &str, instrument_type: InstrumentType) -> Result<MarketData> {
        let data = self.get_many(&[symbol], instrument_type).await?;
        data.into_iter()
            .next()
            .ok_or_else(|| Error::InvalidSymbol(symbol.to_string()))
    }

    /// Get market data for multiple symbols.
    ///
    /// Note: There is a combined limit of 100 symbols per request across
    /// all instrument types.
    pub async fn get_many(
        &self,
        symbols: &[&str],
        instrument_type: InstrumentType,
    ) -> Result<Vec<MarketData>> {
        if symbols.len() > MAX_SYMBOLS_PER_REQUEST {
            return Err(Error::InvalidInput(format!(
                "Too many symbols. Maximum is {}, got {}",
                MAX_SYMBOLS_PER_REQUEST,
                symbols.len()
            )));
        }

        #[derive(Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Query {
            symbols: String,
            instrument_type: InstrumentType,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<MarketData>,
        }

        let query = Query {
            symbols: symbols.join(","),
            instrument_type,
        };

        let response: Response = self
            .inner
            .get_with_query("/market-data", &query)
            .await?;
        Ok(response.items)
    }

    /// Get market data for equities.
    pub async fn equities(&self, symbols: &[&str]) -> Result<Vec<MarketData>> {
        self.get_many(symbols, InstrumentType::Equity).await
    }

    /// Get market data for equity options.
    pub async fn options(&self, symbols: &[&str]) -> Result<Vec<MarketData>> {
        self.get_many(symbols, InstrumentType::EquityOption).await
    }

    /// Get market data for futures.
    pub async fn futures(&self, symbols: &[&str]) -> Result<Vec<MarketData>> {
        self.get_many(symbols, InstrumentType::Future).await
    }

    /// Get market data for cryptocurrencies.
    pub async fn crypto(&self, symbols: &[&str]) -> Result<Vec<MarketData>> {
        self.get_many(symbols, InstrumentType::Cryptocurrency)
            .await
    }

    /// Get detailed quote with Greeks for a single symbol using /market-data/by-type.
    ///
    /// This endpoint returns more detailed data including option Greeks
    /// (delta, gamma, theta, vega, rho) when querying option contracts.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
    /// // Get detailed quote with Greeks for an option
    /// let quote = client.market_data()
    ///     .get_detailed("AAPL  250117C00200000", "Equity Option")
    ///     .await?;
    /// println!("Delta: {:?}, Gamma: {:?}", quote.delta, quote.gamma);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_detailed(
        &self,
        symbol: &str,
        instrument_type: &str,
    ) -> Result<DetailedQuote> {
        let quotes = self.get_detailed_many(&[symbol], instrument_type).await?;
        quotes
            .into_iter()
            .next()
            .ok_or_else(|| Error::InvalidSymbol(symbol.to_string()))
    }

    /// Get detailed quotes with Greeks for multiple symbols using /market-data/by-type.
    ///
    /// This endpoint returns more detailed data including option Greeks
    /// (delta, gamma, theta, vega, rho) when querying option contracts.
    ///
    /// # Arguments
    ///
    /// * `symbols` - List of symbols to query
    /// * `instrument_type` - The instrument type string (e.g., "Equity Option", "Equity", "Future")
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example(client: tastytrade_rs::TastytradeClient) -> tastytrade_rs::Result<()> {
    /// // Get detailed quotes with Greeks for multiple options
    /// let quotes = client.market_data()
    ///     .get_detailed_many(
    ///         &["AAPL  250117C00200000", "AAPL  250117P00200000"],
    ///         "Equity Option"
    ///     )
    ///     .await?;
    /// for quote in quotes {
    ///     println!("{}: delta={:?}, theta={:?}", quote.symbol, quote.delta, quote.theta);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_detailed_many(
        &self,
        symbols: &[&str],
        instrument_type: &str,
    ) -> Result<Vec<DetailedQuote>> {
        if symbols.len() > MAX_SYMBOLS_PER_REQUEST {
            return Err(Error::InvalidInput(format!(
                "Too many symbols. Maximum is {}, got {}",
                MAX_SYMBOLS_PER_REQUEST,
                symbols.len()
            )));
        }

        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<DetailedQuote>,
        }

        // Convert instrument type to API parameter name format
        // e.g., "Equity Option" -> "equity-option[]", "Equity" -> "equity[]"
        let param_name = format!(
            "{}[]",
            instrument_type.to_lowercase().replace(' ', "-")
        );

        // Build query string with the instrument-type-based parameter name
        let symbols_query: String = symbols
            .iter()
            .map(|s| format!("{}={}", param_name, urlencoding::encode(s)))
            .collect::<Vec<_>>()
            .join("&");

        let path = format!("/market-data/by-type?{}", symbols_query);

        let response: Response = self.inner.get(&path).await?;
        Ok(response.items)
    }

    /// Get detailed option quotes with Greeks.
    ///
    /// Convenience method for getting detailed quotes for equity options.
    pub async fn options_detailed(&self, symbols: &[&str]) -> Result<Vec<DetailedQuote>> {
        self.get_detailed_many(symbols, "Equity Option").await
    }

    /// Get detailed equity quotes.
    ///
    /// Convenience method for getting detailed quotes for equities.
    pub async fn equities_detailed(&self, symbols: &[&str]) -> Result<Vec<DetailedQuote>> {
        self.get_detailed_many(symbols, "Equity").await
    }

    /// Get detailed futures quotes.
    ///
    /// Convenience method for getting detailed quotes for futures.
    pub async fn futures_detailed(&self, symbols: &[&str]) -> Result<Vec<DetailedQuote>> {
        self.get_detailed_many(symbols, "Future").await
    }
}
