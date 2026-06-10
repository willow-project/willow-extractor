//! CrossRef API for DOI resolution

use super::FetchError;
use crate::{Config, Item};

/// Fetch metadata for a DOI from CrossRef
pub async fn fetch_doi(doi: &str, config: &Config) -> Result<Item, FetchError> {
    let url = format!("https://api.crossref.org/works/{}", doi);

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
        return Err(FetchError::NotFound(doi.to_string()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| FetchError::Parse(e.to_string()))?;

    parse_response(&data, doi)
}

fn parse_response(data: &serde_json::Value, doi: &str) -> Result<Item, FetchError> {
    let msg = data.get("message").ok_or_else(|| FetchError::Parse("no message".to_string()))?;

    let title = msg
        .get("title")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.first())
        .and_then(|t| t.as_str())
        .map(String::from);

    let authors = msg
        .get("author")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|author| {
                    let given = author.get("given").and_then(|g| g.as_str()).unwrap_or("");
                    let family = author.get("family").and_then(|f| f.as_str()).unwrap_or("");
                    if family.is_empty() {
                        None
                    } else if given.is_empty() {
                        Some(family.to_string())
                    } else {
                        Some(format!("{} {}", given, family))
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let date = msg
        .get("published-print")
        .or_else(|| msg.get("published-online"))
        .or_else(|| msg.get("created"))
        .and_then(|d| d.get("date-parts"))
        .and_then(|dp| dp.as_array())
        .and_then(|arr| arr.first())
        .and_then(|parts| parts.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.as_i64().map(|n| n.to_string()))
                .collect::<Vec<_>>()
                .join("-")
        });

    let publisher = msg.get("publisher").and_then(|p| p.as_str()).map(String::from);
    let url = msg.get("URL").and_then(|u| u.as_str()).map(String::from);

    let item_type = msg.get("type").and_then(|t| t.as_str()).map(|t| {
        match t {
            "journal-article" => "journalArticle",
            "book" => "book",
            "book-chapter" => "bookSection",
            "proceedings-article" => "conferencePaper",
            _ => t,
        }
        .to_string()
    });

    Ok(Item {
        title,
        authors,
        date,
        url,
        doi: Some(doi.to_string()),
        publisher,
        item_type,
        source: Some("crossref".to_string()),
        ..Default::default()
    })
}
