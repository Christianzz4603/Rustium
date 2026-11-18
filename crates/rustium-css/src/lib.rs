//! CSS tokenizer for the Rustium engine, following the CSS Syntax Module
//! Level 3 tokenization algorithm.
//!
//! Scope: tokenization only. There is no parser, no selectors and no values
//! yet (see docs/ROADMAP.md).
//!
//! Behaviour notes:
//! - Input preprocessing: CR, CRLF and FF become LF, and NUL becomes U+FFFD.
//! - Comments are consumed and produce no token.
//! - Every parse error the spec names is recorded in [`Tokenized::errors`]
//!   (the tokenizer always recovers and keeps going, as the spec requires).
//! - Token positions are not recorded yet; when they are added they will refer
//!   to the preprocessed text, because preprocessing can change byte lengths.
//! - Numbers that overflow `f64` are clamped to `±f64::MAX`.

mod tokenizer;

pub use tokenizer::{tokenize, NumericKind, Token, Tokenized};
