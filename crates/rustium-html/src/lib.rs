//! HTML parsing for Rustium: tokenizer plus tree builder.
//!
//! ```
//! let result = rustium_html::parse_document("<p>Hello <b>world</b>");
//! assert_eq!(
//!     result.dom.inner_html(result.dom.document()),
//!     "<html><head></head><body><p>Hello <b>world</b></p></body></html>"
//! );
//! ```
//!
//! Conformance status: this follows the HTML Living Standard's parsing
//! algorithm for the modes listed in `tree_builder.rs`. Measured pass rates
//! against the web-platform-tests html5lib vectors are in `docs/CONFORMANCE.md`
//! (not 100%), so no standards-compliance claim is made. Known gaps are
//! reported as `Unsupported` diagnostics where practical.

mod entities;
mod entity_table;
mod formatting;
mod frameset;
mod in_body;
mod quirks;
mod tables;
mod token;
mod tokenizer;
mod tree_builder;

use std::borrow::Cow;

use rustium_common::Diagnostic;
use rustium_dom::{Dom, NodeId};

pub use entities::{decode_char_ref, CharRef};
pub use token::{SpannedToken, Token};
pub use tokenizer::{raw_kind_for, tokenize, RawKind, Tokenizer};
pub use tree_builder::MAX_OPEN_DEPTH;

use tree_builder::TreeBuilder;

/// Parser configuration.
#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    /// Whether scripting is enabled. When true (the default, matching a
    /// JS-capable browser) `<noscript>` content is parsed as raw text.
    pub scripting: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions { scripting: true }
    }
}

/// Output of a parse.
#[derive(Debug)]
pub struct ParseResult {
    /// The resulting tree.
    pub dom: Dom,
    /// The Document node (for [`parse_document`]) or a DocumentFragment (for [`parse_fragment`]).
    pub root: NodeId,
    /// Parse errors and unsupported-feature reports, ordered by source offset.
    /// Spans refer to the newline-normalized input.
    pub diagnostics: Vec<Diagnostic>,
}

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    if s.contains('\r') {
        Cow::Owned(s.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(s)
    }
}

fn run(input: &str, mut tb: TreeBuilder, mut tz: Tokenizer<'_>) -> (Dom, Vec<Diagnostic>) {
    loop {
        let st = tz.next_token();
        tb.span = st.span;
        let eof = st.token == Token::Eof;
        tb.process(st.token);
        if eof {
            break;
        }
    }
    let _ = input;
    let mut diags = tz.take_diagnostics();
    diags.append(&mut tb.diags);
    diags.sort_by_key(|d| d.span.start);
    (tb.dom, diags)
}

/// Parses a complete HTML document with default options.
pub fn parse_document(input: &str) -> ParseResult {
    parse_document_with(input, &ParseOptions::default())
}

/// Parses a complete HTML document.
pub fn parse_document_with(input: &str, opts: &ParseOptions) -> ParseResult {
    let input = normalize_newlines(input);
    let tz = Tokenizer::new(&input, opts.scripting);
    let tb = TreeBuilder::new(Dom::new(), opts.scripting);
    let (dom, diagnostics) = run(&input, tb, tz);
    let root = dom.document();
    ParseResult {
        dom,
        root,
        diagnostics,
    }
}

/// Parses an HTML fragment as if it were the children of a `context` element
/// (e.g. `"div"`). The result's `root` is a DocumentFragment.
pub fn parse_fragment(input: &str, context: &str) -> ParseResult {
    parse_fragment_with(input, context, &ParseOptions::default())
}

/// Fragment parsing with explicit options.
pub fn parse_fragment_with(input: &str, context: &str, opts: &ParseOptions) -> ParseResult {
    let input = normalize_newlines(input);
    let context = context.to_ascii_lowercase();
    let mut tz = Tokenizer::new(&input, opts.scripting);
    if let Some(kind) = raw_kind_for(&context, opts.scripting) {
        tz.set_raw(kind, &context);
    }
    let mut tb = TreeBuilder::new(Dom::new(), opts.scripting);
    tb.create_root_html(&[]);
    let html = tb.open[0];
    tb.context = Some(context.clone());
    tb.reset_insertion_mode();
    let (mut dom, diagnostics) = run(&input, tb, tz);
    let frag = dom.create_fragment();
    let kids: Vec<NodeId> = dom.children(html).collect();
    for k in kids {
        let _ = dom.append_child(frag, k);
    }
    dom.detach(html);
    ParseResult {
        dom,
        root: frag,
        diagnostics,
    }
}
