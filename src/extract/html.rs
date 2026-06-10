//! HTML metadata extraction
//!
//! Extracts metadata from JSON-LD, OpenGraph, Dublin Core, Highwire Press tags, and standard meta tags.

use crate::Item;
use scraper::{Html, Selector};

/// Extract metadata from HTML content
pub fn extract_html(html: &str) -> Vec<Item> {
    let doc = Html::parse_document(html);

    // Try extractors in priority order
    if let Some(item) = extract_jsonld(&doc) {
        return vec![item];
    }
    if let Some(item) = extract_highwire(&doc) {
        return vec![item];
    }
    if let Some(item) = extract_dublincore(&doc) {
        return vec![item];
    }
    if let Some(item) = extract_opengraph(&doc) {
        return vec![item];
    }
    if let Some(item) = extract_meta(&doc) {
        return vec![item];
    }

    vec![]
}

/// Extract from JSON-LD structured data
fn extract_jsonld(doc: &Html) -> Option<Item> {
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;

    for script in doc.select(&selector) {
        let json_text = script.text().collect::<String>();
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_text) {
            if let Some(item) = parse_jsonld_value(&data) {
                return Some(item);
            }
        }
    }
    None
}

fn parse_jsonld_value(data: &serde_json::Value) -> Option<Item> {
    // Handle @graph arrays
    if let Some(graph) = data.get("@graph").and_then(|g| g.as_array()) {
        for entry in graph {
            if let Some(item) = parse_jsonld_object(entry) {
                return Some(item);
            }
        }
    }

    // Handle arrays
    if let Some(arr) = data.as_array() {
        for entry in arr {
            if let Some(item) = parse_jsonld_object(entry) {
                return Some(item);
            }
        }
    }

    parse_jsonld_object(data)
}

fn parse_jsonld_object(obj: &serde_json::Value) -> Option<Item> {
    let type_val = obj.get("@type")?;
    let item_type = match type_val {
        serde_json::Value::String(s) => s.as_str(),
        serde_json::Value::Array(arr) => arr.first()?.as_str()?,
        _ => return None,
    };

    // Only process relevant types
    let normalized_type = match item_type {
        "Article" | "NewsArticle" | "ScholarlyArticle" | "TechArticle" => "journalArticle",
        "Book" => "book",
        "WebPage" | "WebSite" => "webpage",
        "VideoObject" => "video",
        "BlogPosting" => "blogPost",
        _ => return None,
    };

    let title = obj
        .get("headline")
        .or_else(|| obj.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let authors = extract_jsonld_authors(obj);

    let date = obj
        .get("datePublished")
        .or_else(|| obj.get("dateCreated"))
        .and_then(|v| v.as_str())
        .map(|s| s.chars().take(10).collect());

    let url = obj.get("url").and_then(|v| v.as_str()).map(String::from);

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let publisher = obj
        .get("publisher")
        .and_then(|p| p.get("name").or(Some(p)))
        .and_then(|v| v.as_str())
        .map(String::from);

    if title.is_none() && authors.is_empty() {
        return None;
    }

    Some(Item {
        title,
        authors,
        date,
        url,
        publisher,
        description,
        item_type: Some(normalized_type.to_string()),
        source: Some("jsonld".to_string()),
        ..Default::default()
    })
}

fn extract_jsonld_authors(obj: &serde_json::Value) -> Vec<String> {
    let author_val = match obj.get("author") {
        Some(v) => v,
        None => return vec![],
    };

    let authors_arr = match author_val {
        serde_json::Value::Array(arr) => arr.clone(),
        other => vec![other.clone()],
    };

    authors_arr
        .iter()
        .filter_map(|a| {
            if let Some(name) = a.as_str() {
                Some(name.to_string())
            } else {
                a.get("name").and_then(|n| n.as_str()).map(String::from)
            }
        })
        .collect()
}

/// Extract from Highwire Press meta tags (academic papers)
fn extract_highwire(doc: &Html) -> Option<Item> {
    let title = get_meta(doc, "citation_title")?;

    let authors: Vec<String> = get_all_meta(doc, "citation_author");
    let date = get_meta(doc, "citation_publication_date")
        .or_else(|| get_meta(doc, "citation_date"));
    let doi = get_meta(doc, "citation_doi");
    let publisher = get_meta(doc, "citation_publisher");

    Some(Item {
        title: Some(title),
        authors,
        date,
        doi,
        publisher,
        item_type: Some("journalArticle".to_string()),
        source: Some("highwire".to_string()),
        ..Default::default()
    })
}

/// Extract from Dublin Core meta tags
fn extract_dublincore(doc: &Html) -> Option<Item> {
    let title = get_meta(doc, "DC.title").or_else(|| get_meta(doc, "dc.title"))?;

    let authors = get_all_meta(doc, "DC.creator")
        .into_iter()
        .chain(get_all_meta(doc, "dc.creator"))
        .collect();

    let date = get_meta(doc, "DC.date").or_else(|| get_meta(doc, "dc.date"));
    let publisher = get_meta(doc, "DC.publisher").or_else(|| get_meta(doc, "dc.publisher"));
    let description = get_meta(doc, "DC.description").or_else(|| get_meta(doc, "dc.description"));

    Some(Item {
        title: Some(title),
        authors,
        date,
        publisher,
        description,
        item_type: Some("document".to_string()),
        source: Some("dublincore".to_string()),
        ..Default::default()
    })
}

/// Extract from OpenGraph meta tags
fn extract_opengraph(doc: &Html) -> Option<Item> {
    let title = get_meta(doc, "og:title")?;

    let item_type = get_meta(doc, "og:type").map(|t| {
        if t.starts_with("video") {
            "video"
        } else {
            match t.as_str() {
                "article" => "journalArticle",
                "book" => "book",
                "music.song" | "music.album" => "audio",
                _ => "webpage",
            }
        }.to_string()
    });

    let description = get_meta(doc, "og:description");
    let url = get_meta(doc, "og:url");

    Some(Item {
        title: Some(title),
        url,
        description,
        item_type,
        source: Some("opengraph".to_string()),
        ..Default::default()
    })
}

/// Extract from standard meta tags
fn extract_meta(doc: &Html) -> Option<Item> {
    // Try title tag first
    let title_selector = Selector::parse("title").ok()?;
    let title = doc
        .select(&title_selector)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())?;

    let description = get_meta(doc, "description");
    let author = get_meta(doc, "author");

    Some(Item {
        title: Some(title),
        authors: author.map(|a| vec![a]).unwrap_or_default(),
        description,
        item_type: Some("webpage".to_string()),
        source: Some("meta".to_string()),
        ..Default::default()
    })
}

/// Get a single meta tag value
fn get_meta(doc: &Html, name: &str) -> Option<String> {
    // Try name attribute
    let name_selector = Selector::parse(&format!(r#"meta[name="{}"]"#, name)).ok()?;
    if let Some(el) = doc.select(&name_selector).next() {
        if let Some(content) = el.value().attr("content") {
            let content = content.trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }

    // Try property attribute (for OpenGraph)
    let prop_selector = Selector::parse(&format!(r#"meta[property="{}"]"#, name)).ok()?;
    if let Some(el) = doc.select(&prop_selector).next() {
        if let Some(content) = el.value().attr("content") {
            let content = content.trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }

    None
}

/// Get all meta tag values with the same name
fn get_all_meta(doc: &Html, name: &str) -> Vec<String> {
    let mut values = Vec::new();

    if let Ok(selector) = Selector::parse(&format!(r#"meta[name="{}"]"#, name)) {
        for el in doc.select(&selector) {
            if let Some(content) = el.value().attr("content") {
                let content = content.trim();
                if !content.is_empty() {
                    values.push(content.to_string());
                }
            }
        }
    }

    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_jsonld() {
        let html = r#"
        <html>
        <head>
        <script type="application/ld+json">
        {
            "@type": "Article",
            "headline": "Test Article",
            "author": {"name": "John Doe"},
            "datePublished": "2024-01-15"
        }
        </script>
        </head>
        </html>
        "#;

        let items = extract_html(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, Some("Test Article".to_string()));
        assert_eq!(items[0].authors, vec!["John Doe"]);
    }

    #[test]
    fn test_extract_opengraph() {
        let html = r#"
        <html>
        <head>
        <meta property="og:title" content="OG Title">
        <meta property="og:type" content="article">
        <meta property="og:description" content="Description here">
        </head>
        </html>
        "#;

        let items = extract_html(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, Some("OG Title".to_string()));
    }

    #[test]
    fn test_extract_highwire() {
        let html = r#"
        <html>
        <head>
        <meta name="citation_title" content="Academic Paper">
        <meta name="citation_author" content="Alice Smith">
        <meta name="citation_author" content="Bob Jones">
        <meta name="citation_doi" content="10.1000/test">
        </head>
        </html>
        "#;

        let items = extract_html(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, Some("Academic Paper".to_string()));
        assert_eq!(items[0].authors.len(), 2);
        assert_eq!(items[0].doi, Some("10.1000/test".to_string()));
    }
}
