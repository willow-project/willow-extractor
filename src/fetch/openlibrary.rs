//! OpenLibrary API for ISBN resolution

use super::FetchError;
use crate::{Config, Item};

/// Fetch metadata for an ISBN from OpenLibrary
pub async fn fetch_isbn(isbn: &str, config: &Config) -> Result<Item, FetchError> {
    let url = format!(
        "https://openlibrary.org/api/books?bibkeys=ISBN:{}&format=json&jscmd=data",
        isbn
    );

    let client = reqwest::Client::builder()
        .timeout(config.api_timeout)
        .build()
        .map_err(|e| FetchError::Network(e.to_string()))?;

    let response = client
        .get(&url)
        .header("User-Agent", &config.user_agent)
        .send()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(FetchError::NotFound(isbn.to_string()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| FetchError::Parse(e.to_string()))?;

    parse_response(&data, isbn)
}

fn parse_response(data: &serde_json::Value, isbn: &str) -> Result<Item, FetchError> {
    let key = format!("ISBN:{}", isbn);
    let book = data.get(&key).ok_or_else(|| FetchError::NotFound(isbn.to_string()))?;

    let title = book.get("title").and_then(|t| t.as_str()).map(String::from);

    let authors = book
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|author| author.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let publisher = book
        .get("publishers")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from);

    let date = book.get("publish_date").and_then(|d| d.as_str()).map(String::from);
    let url = book.get("url").and_then(|u| u.as_str()).map(String::from);

    Ok(Item {
        title,
        authors,
        date,
        url,
        isbn: Some(isbn.to_string()),
        publisher,
        item_type: Some("book".to_string()),
        source: Some("openlibrary".to_string()),
        ..Default::default()
    })
}
