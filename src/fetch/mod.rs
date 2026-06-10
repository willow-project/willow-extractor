//! External API fetchers for resolving identifiers

mod crossref;
mod openlibrary;

use crate::detect::Identifier;
use crate::{Config, Item};

pub use crossref::fetch_doi;
pub use openlibrary::fetch_isbn;

/// Error from fetching
#[derive(Debug)]
pub enum FetchError {
    Network(String),
    NotFound(String),
    Parse(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(e) => write!(f, "network error: {}", e),
            FetchError::NotFound(id) => write!(f, "not found: {}", id),
            FetchError::Parse(e) => write!(f, "parse error: {}", e),
        }
    }
}

impl std::error::Error for FetchError {}

/// Resolve an identifier to metadata
pub async fn resolve(id: &Identifier, config: &Config) -> Option<Item> {
    match id {
        Identifier::Doi(doi) => fetch_doi(doi, config).await.ok(),
        Identifier::Isbn(isbn) => fetch_isbn(isbn, config).await.ok(),
        // TODO: arxiv, pubmed
        _ => None,
    }
}

/// Resolve first matching identifier
pub async fn resolve_any(ids: &[Identifier], config: &Config) -> Option<Item> {
    for id in ids {
        if let Some(item) = resolve(id, config).await {
            return Some(item);
        }
    }
    None
}
