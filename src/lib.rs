//! # Willow Extractor
//!
//! Bibliographic metadata extraction from URLs, HTML, and PDFs.

pub mod detect;
pub mod extract;
pub mod fetch;
pub mod item;

pub use detect::Identifier;
pub use item::Item;

use std::time::Duration;
use thiserror::Error;

/// Configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub request_timeout: Duration,
    pub api_timeout: Duration,
    pub user_agent: String,
    pub max_pdf_pages: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            api_timeout: Duration::from_secs(10),
            user_agent: "Mozilla/5.0 (compatible; Willow/1.0)".to_string(),
            max_pdf_pages: 5,
        }
    }
}

/// Extraction error
#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("network: {0}")]
    Network(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("timeout")]
    Timeout,
}

impl From<reqwest::Error> for ExtractError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            ExtractError::Timeout
        } else {
            ExtractError::Network(e.to_string())
        }
    }
}

/// Extract metadata from a URL
pub async fn extract_from_url(url: &str, config: &Config) -> Result<Vec<Item>, ExtractError> {
    let is_pdf_url = url.to_lowercase().contains(".pdf");

    let client = reqwest::Client::builder()
        .timeout(config.request_timeout)
        .build()?;

    let response = client
        .get(url)
        .header("User-Agent", &config.user_agent)
        .send()
        .await?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("text/html");

    let is_pdf = content_type.contains("application/pdf") || is_pdf_url;

    if is_pdf {
        let bytes = response.bytes().await?;
        if let Some(item) = extract::extract_pdf(&bytes, Some(url), config).await {
            return Ok(vec![item]);
        }
        return Ok(vec![]);
    }

    // HTML
    let html = response.text().await?;
    extract_html(&html, Some(url), config).await
}

/// Extract metadata from HTML
pub async fn extract_html(
    html: &str,
    source_url: Option<&str>,
    config: &Config,
) -> Result<Vec<Item>, ExtractError> {
    let mut items = extract::extract_html(html);

    // If sparse, try to resolve identifiers
    let has_good_metadata = items
        .iter()
        .any(|item| item.title.is_some() && (!item.authors.is_empty() || item.date.is_some()));

    if !has_good_metadata {
        let mut identifiers = detect::detect_all(html);

        // Check URL for DOI
        if let Some(url) = source_url {
            if let Some(doi) = detect::detect_doi(url) {
                identifiers.insert(0, Identifier::Doi(doi));
            }
        }

        if let Some(resolved) = fetch::resolve_any(&identifiers, config).await {
            items.push(resolved);
        }
    }

    Ok(items)
}

/// Resolve an identifier (DOI, ISBN) to metadata
pub async fn resolve_identifier(id: &str, config: &Config) -> Option<Item> {
    if let Some(doi) = detect::detect_doi(id) {
        return fetch::resolve(&Identifier::Doi(doi), config).await;
    }
    if let Some(isbn) = detect::detect_isbn(id) {
        return fetch::resolve(&Identifier::Isbn(isbn), config).await;
    }

    let identifiers = detect::detect_all(id);
    fetch::resolve_any(&identifiers, config).await
}
