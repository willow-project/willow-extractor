//! PDF metadata extraction

use crate::detect;
use crate::fetch;
use crate::{Config, Item};
use lopdf::Document;

/// Extract metadata from PDF bytes
pub async fn extract_pdf(bytes: &[u8], source_url: Option<&str>, config: &Config) -> Option<Item> {
    let doc = match Document::load_mem(bytes) {
        Ok(d) => d,
        Err(_) => return extract_from_filename(source_url?),
    };

    // Try PDF metadata first
    if let Some(item) = extract_pdf_metadata(&doc, source_url) {
        if item.title.is_some() && !item.authors.is_empty() {
            return Some(item);
        }
    }

    // Extract text and look for identifiers
    let text = extract_text(&doc, config.max_pdf_pages);
    if !text.is_empty() {
        let identifiers = detect::detect_all(&text);
        if let Some(item) = fetch::resolve_any(&identifiers, config).await {
            return Some(item);
        }
    }

    // Return partial metadata if available
    if let Some(item) = extract_pdf_metadata(&doc, source_url) {
        if item.title.is_some() || !item.authors.is_empty() {
            return Some(item);
        }
    }

    // Fallback to filename
    extract_from_filename(source_url?)
}

fn extract_text(doc: &Document, max_pages: usize) -> String {
    let page_count = doc.get_pages().len();
    let pages: Vec<u32> = (1..=max_pages.min(page_count) as u32).collect();
    doc.extract_text(&pages).unwrap_or_default()
}

fn extract_pdf_metadata(doc: &Document, source_url: Option<&str>) -> Option<Item> {
    let trailer = &doc.trailer;
    let info_ref = trailer.get(b"Info").ok()?.as_reference().ok()?;
    let info = doc.get_dictionary(info_ref).ok()?;

    let title = get_pdf_string(info, b"Title");
    let author = get_pdf_string(info, b"Author");
    let creation_date = get_pdf_string(info, b"CreationDate");

    let authors = author
        .map(|a| {
            a.split([',', ';', '&'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let date = creation_date.and_then(|d| {
        if d.starts_with("D:") && d.len() >= 6 {
            Some(d[2..6].to_string())
        } else if d.len() >= 4 && d.chars().take(4).all(|c| c.is_ascii_digit()) {
            Some(d[..4].to_string())
        } else {
            None
        }
    });

    Some(Item {
        title,
        authors,
        date,
        url: source_url.map(String::from),
        item_type: Some("document".to_string()),
        source: Some("pdf".to_string()),
        ..Default::default()
    })
}

fn get_pdf_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key)
        .ok()
        .and_then(|obj| match obj {
            lopdf::Object::String(bytes, _) => {
                if bytes.starts_with(&[0xFE, 0xFF]) {
                    // UTF-16BE
                    let utf16: Vec<u16> = bytes[2..]
                        .chunks(2)
                        .filter_map(|c| {
                            if c.len() == 2 {
                                Some(u16::from_be_bytes([c[0], c[1]]))
                            } else {
                                None
                            }
                        })
                        .collect();
                    String::from_utf16(&utf16).ok()
                } else {
                    String::from_utf8(bytes.clone())
                        .ok()
                        .or_else(|| Some(bytes.iter().map(|&c| c as char).collect()))
                }
            }
            lopdf::Object::Name(name) => String::from_utf8(name.clone()).ok(),
            _ => None,
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_from_filename(url: &str) -> Option<Item> {
    let filename = url.split('/').next_back()?.split('?').next()?;
    let filename = urlencoding::decode(filename).ok()?;
    let name = filename.strip_suffix(".pdf").or_else(|| filename.strip_suffix(".PDF"))?;

    // Pattern: "Author - Title - Year" or "Author - Title (Year)"
    let re = regex::Regex::new(r"^(.+?)\s*[-–—]\s*(.+?)\s*[-–—(]\s*(\d{4})\)?$").ok()?;
    if let Some(caps) = re.captures(name) {
        return Some(Item {
            title: caps.get(2).map(|m| m.as_str().trim().to_string()),
            authors: caps.get(1).map(|m| vec![m.as_str().trim().to_string()]).unwrap_or_default(),
            date: caps.get(3).map(|m| m.as_str().to_string()),
            url: Some(url.to_string()),
            item_type: Some("document".to_string()),
            source: Some("filename".to_string()),
            ..Default::default()
        });
    }

    // Pattern: "Title - Author"
    let re2 = regex::Regex::new(r"^(.+?)\s*[-–—]\s*(.+?)$").ok()?;
    if let Some(caps) = re2.captures(name) {
        return Some(Item {
            title: caps.get(1).map(|m| m.as_str().trim().to_string()),
            authors: caps.get(2).map(|m| vec![m.as_str().trim().to_string()]).unwrap_or_default(),
            url: Some(url.to_string()),
            item_type: Some("document".to_string()),
            source: Some("filename".to_string()),
            ..Default::default()
        });
    }

    // Fallback: filename as title
    Some(Item {
        title: Some(name.to_string()),
        url: Some(url.to_string()),
        item_type: Some("document".to_string()),
        source: Some("filename".to_string()),
        ..Default::default()
    })
}
