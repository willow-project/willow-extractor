//! Identifier detection for DOIs, ISBNs, PMIDs, and arXiv IDs

use regex::Regex;
use std::sync::LazyLock;

/// A detected identifier from text
#[derive(Debug, Clone, PartialEq)]
pub enum Identifier {
    Doi(String),
    Isbn(String),
    Pmid(String),
    Arxiv(String),
}

static DOI_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // DOI pattern: 10.XXXX/anything-until-whitespace-or-punctuation
    Regex::new(r#"10\.\d{4,}/[^\s\]>"']+"#).unwrap()
});

static ISBN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // ISBN must have explicit ISBN prefix to avoid false positives
    Regex::new(r"ISBN[:\s-]?(?:97[89][- ]?)?(?:\d[- ]?){9}[\dXx]").unwrap()
});

static PMID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // PubMed ID: PMID followed by numbers
    Regex::new(r"PMID[:\s]*(\d+)").unwrap()
});

static ARXIV_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // arXiv ID: old format (hep-th/9901001) or new format (1234.56789)
    Regex::new(r"arXiv:(\d{4}\.\d{4,5}(?:v\d+)?|[a-z-]+/\d{7})").unwrap()
});

/// Detect all identifiers in text
pub fn detect_all(text: &str) -> Vec<Identifier> {
    let mut identifiers = Vec::new();

    // DOIs
    for cap in DOI_REGEX.find_iter(text) {
        let doi = cap.as_str().trim_end_matches(['.', ',', ';']);
        identifiers.push(Identifier::Doi(doi.to_string()));
    }

    // ISBNs
    for cap in ISBN_REGEX.find_iter(text) {
        let isbn = normalize_isbn(cap.as_str());
        if is_valid_isbn(&isbn) {
            identifiers.push(Identifier::Isbn(isbn));
        }
    }

    // PMIDs
    for cap in PMID_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            identifiers.push(Identifier::Pmid(m.as_str().to_string()));
        }
    }

    // arXiv
    for cap in ARXIV_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            identifiers.push(Identifier::Arxiv(m.as_str().to_string()));
        }
    }

    // Dedupe
    identifiers.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
    identifiers.dedup();

    identifiers
}

/// Detect DOIs specifically
pub fn detect_doi(text: &str) -> Option<String> {
    DOI_REGEX.find(text).map(|m| {
        m.as_str()
            .trim_end_matches(['.', ',', ';'])
            .to_string()
    })
}

/// Detect ISBN specifically
pub fn detect_isbn(text: &str) -> Option<String> {
    ISBN_REGEX.find(text).and_then(|m| {
        let isbn = normalize_isbn(m.as_str());
        if is_valid_isbn(&isbn) {
            Some(isbn)
        } else {
            None
        }
    })
}

fn normalize_isbn(isbn: &str) -> String {
    isbn.chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .collect::<String>()
        .to_uppercase()
}

fn is_valid_isbn(isbn: &str) -> bool {
    match isbn.len() {
        10 => is_valid_isbn10(isbn),
        13 => is_valid_isbn13(isbn),
        _ => false,
    }
}

fn is_valid_isbn10(isbn: &str) -> bool {
    let chars: Vec<char> = isbn.chars().collect();
    let mut sum = 0;

    for (i, c) in chars.iter().enumerate() {
        let digit = if *c == 'X' {
            10
        } else {
            c.to_digit(10).unwrap_or(0) as i32
        };
        sum += digit * (10 - i as i32);
    }

    sum % 11 == 0
}

fn is_valid_isbn13(isbn: &str) -> bool {
    let chars: Vec<char> = isbn.chars().collect();
    let mut sum = 0;

    for (i, c) in chars.iter().enumerate() {
        let digit = c.to_digit(10).unwrap_or(0) as i32;
        sum += digit * if i % 2 == 0 { 1 } else { 3 };
    }

    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_doi() {
        assert_eq!(
            detect_doi("Check out 10.1000/xyz123 for more"),
            Some("10.1000/xyz123".to_string())
        );
        assert_eq!(
            detect_doi("DOI: 10.1038/nature12373."),
            Some("10.1038/nature12373".to_string())
        );
    }

    #[test]
    fn test_detect_isbn() {
        // Valid ISBN-13
        assert_eq!(
            detect_isbn("ISBN 978-0-13-468599-1"),
            Some("9780134685991".to_string())
        );
    }

    #[test]
    fn test_detect_all() {
        let text = "See 10.1000/test and ISBN 978-0-13-468599-1 and PMID: 12345";
        let ids = detect_all(text);
        assert!(ids.contains(&Identifier::Doi("10.1000/test".to_string())));
        assert!(ids.contains(&Identifier::Pmid("12345".to_string())));
    }
}
