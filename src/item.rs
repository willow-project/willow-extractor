//! Bibliographic item type

use serde::{Deserialize, Serialize};

/// A bibliographic item
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Item {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub date: Option<String>,
    pub url: Option<String>,
    pub doi: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub item_type: Option<String>,
    /// Where the metadata came from (e.g., "crossref", "openlibrary", "html")
    pub source: Option<String>,
}

impl Item {
    pub fn is_complete(&self) -> bool {
        self.title.is_some() && !self.authors.is_empty()
    }
}
