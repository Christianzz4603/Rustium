//! Shared types for the Rustium engine.
//!
//! Currently provides source positions, parse diagnostics, and an
//! [`Atom`] type for cheap-to-clone interned-style strings.

use std::fmt;
use std::rc::Rc;

/// A cheaply clonable, immutable string.
///
/// This is a reference-counted string, not a global interner. Cloning is a
/// pointer copy. A true interner is planned (see docs/ARCHITECTURE.md).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Atom(Rc<str>);

impl Atom {
    /// Creates an atom from any string slice.
    pub fn new(s: &str) -> Self {
        Atom(Rc::from(s))
    }

    /// Returns the string contents.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self.0)
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Atom {
    fn from(s: &str) -> Self {
        Atom::new(s)
    }
}

impl PartialEq<str> for Atom {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for Atom {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

/// A byte offset range into source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

/// Severity of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Spec-defined parse error that the parser recovered from.
    ParseError,
    /// Valid input that Rustium does not yet support.
    Unsupported,
}

/// A recoverable problem found while processing input.
///
/// Rustium never silently ignores unsupported or malformed input; it is
/// reported here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity class.
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `unexpected-null-character`.
    pub code: &'static str,
    /// Location in the source, if known.
    pub span: Span,
}

impl Diagnostic {
    /// Creates a parse-error diagnostic.
    pub fn parse_error(code: &'static str, span: Span) -> Self {
        Diagnostic {
            severity: Severity::ParseError,
            code,
            span,
        }
    }

    /// Creates an unsupported-feature diagnostic.
    pub fn unsupported(code: &'static str, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Unsupported,
            code,
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_clone_and_eq() {
        let a = Atom::new("div");
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(a, "div");
        assert_eq!(a.as_str(), "div");
        assert_eq!(format!("{a}"), "div");
    }

    #[test]
    fn diagnostic_constructors() {
        let d = Diagnostic::parse_error("x", Span { start: 1, end: 2 });
        assert_eq!(d.severity, Severity::ParseError);
        let u = Diagnostic::unsupported("y", Span::default());
        assert_eq!(u.severity, Severity::Unsupported);
    }
}
