//! Rustium engine facade.
//!
//! This is the crate embedders (e.g. the future Axylum browser) depend on.
//! The API is **unstable** until Phase 4; today it exposes HTML parsing into a
//! DOM and nothing else. Later phases add style, layout, and rendering here.

pub use rustium_common as common;
pub use rustium_dom as dom;
pub use rustium_html as html;

use rustium_common::Diagnostic;
use rustium_dom::Dom;

/// Engine version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A parsed document and the problems found while parsing it.
#[derive(Debug)]
pub struct Document {
    /// The DOM tree.
    pub dom: Dom,
    /// Parse errors and unsupported-feature reports.
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    /// Parses an HTML document.
    pub fn parse(html: &str) -> Document {
        let r = rustium_html::parse_document(html);
        Document {
            dom: r.dom,
            diagnostics: r.diagnostics,
        }
    }

    /// Serializes the document back to HTML.
    pub fn to_html(&self) -> String {
        self.dom.inner_html(self.dom.document())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_serialize() {
        let d = Document::parse("<!DOCTYPE html><title>t</title><p>hi");
        assert_eq!(
            d.to_html(),
            "<!DOCTYPE html><html><head><title>t</title></head><body><p>hi</p></body></html>"
        );
    }

    #[test]
    fn version_is_set() {
        assert!(!VERSION.is_empty());
    }
}
