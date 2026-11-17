//! Shared types for the Rustium engine.
//!
//! Currently provides source positions, parse diagnostics, and an
//! [`Atom`] type for cheap-to-clone interned-style strings.

use std::fmt;
use std::rc::Rc;

/// Common HTML element and attribute names, kept sorted for binary search.
/// Atoms for these names borrow static data and never allocate.
const STATIC_ATOMS: &[&str] = &[
    "a",
    "abbr",
    "action",
    "address",
    "align",
    "alt",
    "area",
    "article",
    "aside",
    "async",
    "audio",
    "b",
    "base",
    "big",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "center",
    "charset",
    "checked",
    "class",
    "code",
    "col",
    "colgroup",
    "color",
    "cols",
    "colspan",
    "content",
    "data",
    "dd",
    "defer",
    "details",
    "dialog",
    "dir",
    "disabled",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "face",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "for",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "height",
    "hidden",
    "hr",
    "href",
    "html",
    "http-equiv",
    "i",
    "id",
    "iframe",
    "img",
    "input",
    "label",
    "lang",
    "legend",
    "li",
    "link",
    "main",
    "math",
    "media",
    "menu",
    "meta",
    "method",
    "name",
    "nav",
    "nobr",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "placeholder",
    "pre",
    "readonly",
    "rel",
    "rows",
    "rowspan",
    "s",
    "script",
    "section",
    "select",
    "selected",
    "size",
    "small",
    "source",
    "span",
    "src",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "svg",
    "table",
    "target",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "tt",
    "type",
    "u",
    "ul",
    "value",
    "video",
    "width",
];

/// A cheaply clonable, immutable string.
///
/// Common HTML names are borrowed from static data (no allocation); anything
/// else is a reference-counted string. Cloning is a pointer copy either way.
/// This is not a global interner; a true interner is planned (see
/// docs/ARCHITECTURE.md). Equality, ordering and hashing are by string
/// contents, so the two representations are interchangeable.
#[derive(Clone)]
pub struct Atom(Repr);

#[derive(Clone)]
enum Repr {
    Static(&'static str),
    Shared(Rc<str>),
}

impl Default for Atom {
    fn default() -> Self {
        Atom(Repr::Static(""))
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Atom {}

impl std::hash::Hash for Atom {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for Atom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Atom {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// `ATOM_BUCKETS[b]..ATOM_BUCKETS[b + 1]` is the range of `STATIC_ATOMS` whose
/// first byte is `b`. Computed at compile time from the sorted table.
const ATOM_BUCKETS: [u8; 257] = {
    let mut t = [0u8; 257];
    let mut i = 0;
    let mut b = 0;
    while b <= 256 {
        // Advance i to the first entry whose first byte is >= b.
        while i < STATIC_ATOMS.len() && (STATIC_ATOMS[i].as_bytes()[0] as usize) < b {
            i += 1;
        }
        t[b] = i as u8;
        b += 1;
    }
    t
};

impl Atom {
    /// Creates an atom from any string slice. Common HTML names do not allocate.
    pub fn new(s: &str) -> Self {
        // Narrow the binary search to the names sharing the first byte.
        let Some(&first) = s.as_bytes().first() else {
            return Atom(Repr::Shared(Rc::from(s)));
        };
        let lo = ATOM_BUCKETS[first as usize] as usize;
        let hi = ATOM_BUCKETS[first as usize + 1] as usize;
        match STATIC_ATOMS[lo..hi].binary_search(&s) {
            Ok(i) => Atom(Repr::Static(STATIC_ATOMS[lo + i])),
            Err(_) => Atom(Repr::Shared(Rc::from(s))),
        }
    }

    /// Returns the string contents.
    pub fn as_str(&self) -> &str {
        match &self.0 {
            Repr::Static(s) => s,
            Repr::Shared(s) => s,
        }
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Atom {
    fn from(s: &str) -> Self {
        Atom::new(s)
    }
}

impl PartialEq<str> for Atom {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Atom {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
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

#[cfg(test)]
mod static_atom_tests {
    use super::*;

    #[test]
    fn static_table_is_sorted_and_unique() {
        assert!(STATIC_ATOMS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn every_static_name_is_found_without_allocating() {
        for name in STATIC_ATOMS {
            assert!(matches!(Atom::new(name).0, Repr::Static(_)), "{name}");
        }
        for name in ["", "zzz", "Div", "\u{e9}t\u{e9}", "custom-element"] {
            assert!(matches!(Atom::new(name).0, Repr::Shared(_)), "{name}");
        }
    }

    #[test]
    fn static_and_shared_atoms_are_equal_and_hash_alike() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let a = Atom::new("div");
        let b = Atom(Repr::Shared(Rc::from("div")));
        assert_eq!(a, b);
        let h = |x: &Atom| {
            let mut s = DefaultHasher::new();
            x.hash(&mut s);
            s.finish()
        };
        assert_eq!(h(&a), h(&b));
        assert_eq!(Atom::new("custom-element"), "custom-element");
    }
}
