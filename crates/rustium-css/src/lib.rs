//! CSS tokenizer for the Rustium engine, following the CSS Syntax Module
//! Level 3 tokenization algorithm.
//!
//! Scope: tokenization and grouping into component values (blocks and
//! functions). There are no rules, declarations, selectors or values yet (see
//! docs/ROADMAP.md).
//!
//! Behaviour notes:
//! - Input preprocessing: CR, CRLF and FF become LF, and NUL becomes U+FFFD.
//! - Comments are consumed and produce no token.
//! - Every parse error the spec names is recorded in [`Tokenized::errors`]
//!   (the tokenizer always recovers and keeps going, as the spec requires).
//! - Token positions are not recorded yet; when they are added they will refer
//!   to the preprocessed text, because preprocessing can change byte lengths.
//! - Numbers that overflow `f64` are clamped to `±f64::MAX`.

mod component;
mod tokenizer;

pub use component::{
    component_values, parse_component_value_list, BlockKind, ComponentValue, ParsedValues,
};
pub use tokenizer::{tokenize, NumericKind, Token, Tokenized};
