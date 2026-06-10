//! Content extraction from HTML and PDF

mod html;
mod pdf;

pub use html::extract_html;
pub use pdf::extract_pdf;
