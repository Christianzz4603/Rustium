//! Tokens produced by the tokenizer.

use rustium_common::Span;

/// An HTML token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// `<!DOCTYPE name ...>`. Public/system identifiers are not retained.
    Doctype {
        /// Lowercased name, if present.
        name: Option<String>,
    },
    /// A start tag.
    StartTag {
        /// Lowercased tag name.
        name: String,
        /// Attributes in source order; duplicates removed (first wins).
        attrs: Vec<(String, String)>,
        /// Whether the tag ended with `/>`.
        self_closing: bool,
    },
    /// An end tag.
    EndTag {
        /// Lowercased tag name.
        name: String,
    },
    /// A comment.
    Comment(String),
    /// A run of character data.
    Character(String),
    /// End of input.
    Eof,
}

/// A token together with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedToken {
    /// The token.
    pub token: Token,
    /// Source byte range.
    pub span: Span,
}
