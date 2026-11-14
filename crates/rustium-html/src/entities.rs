//! Character reference decoding.
//!
//! Numeric references are fully handled per the HTML Living Standard
//! (including the windows-1252 C1 remapping). Named references cover only a
//! curated subset (~80 of the 2,231 spec entities); see docs/ARCHITECTURE.md.
//! Unknown names are left as literal text and reported as a parse error.

/// A decoded character reference.
#[derive(Debug, PartialEq, Eq)]
pub struct CharRef {
    /// Replacement text.
    pub value: String,
    /// Bytes consumed after the `&`.
    pub consumed: usize,
    /// Parse-error code if the reference was malformed but recovered.
    pub error: Option<&'static str>,
}

/// (name, replacement, allowed without trailing semicolon)
const NAMED: &[(&str, &str, bool)] = &[
    ("amp", "&", true),
    ("AMP", "&", true),
    ("lt", "<", true),
    ("LT", "<", true),
    ("gt", ">", true),
    ("GT", ">", true),
    ("quot", "\"", true),
    ("QUOT", "\"", true),
    ("apos", "'", false),
    ("nbsp", "\u{a0}", true),
    ("copy", "\u{a9}", true),
    ("COPY", "\u{a9}", true),
    ("reg", "\u{ae}", true),
    ("REG", "\u{ae}", true),
    ("trade", "\u{2122}", false),
    ("hellip", "\u{2026}", false),
    ("mdash", "\u{2014}", false),
    ("ndash", "\u{2013}", false),
    ("lsquo", "\u{2018}", false),
    ("rsquo", "\u{2019}", false),
    ("ldquo", "\u{201c}", false),
    ("rdquo", "\u{201d}", false),
    ("bull", "\u{2022}", false),
    ("middot", "\u{b7}", true),
    ("euro", "\u{20ac}", false),
    ("pound", "\u{a3}", true),
    ("yen", "\u{a5}", true),
    ("cent", "\u{a2}", true),
    ("sect", "\u{a7}", true),
    ("para", "\u{b6}", true),
    ("deg", "\u{b0}", true),
    ("plusmn", "\u{b1}", true),
    ("times", "\u{d7}", true),
    ("divide", "\u{f7}", true),
    ("laquo", "\u{ab}", true),
    ("raquo", "\u{bb}", true),
    ("micro", "\u{b5}", true),
    ("frac12", "\u{bd}", true),
    ("frac14", "\u{bc}", true),
    ("frac34", "\u{be}", true),
    ("sup2", "\u{b2}", true),
    ("sup3", "\u{b3}", true),
    ("larr", "\u{2190}", false),
    ("uarr", "\u{2191}", false),
    ("rarr", "\u{2192}", false),
    ("darr", "\u{2193}", false),
    ("hearts", "\u{2665}", false),
    ("check", "\u{2713}", false),
    ("agrave", "\u{e0}", true),
    ("aacute", "\u{e1}", true),
    ("acirc", "\u{e2}", true),
    ("atilde", "\u{e3}", true),
    ("auml", "\u{e4}", true),
    ("aring", "\u{e5}", true),
    ("ccedil", "\u{e7}", true),
    ("egrave", "\u{e8}", true),
    ("eacute", "\u{e9}", true),
    ("ecirc", "\u{ea}", true),
    ("euml", "\u{eb}", true),
    ("igrave", "\u{ec}", true),
    ("iacute", "\u{ed}", true),
    ("icirc", "\u{ee}", true),
    ("iuml", "\u{ef}", true),
    ("ntilde", "\u{f1}", true),
    ("ograve", "\u{f2}", true),
    ("oacute", "\u{f3}", true),
    ("ocirc", "\u{f4}", true),
    ("otilde", "\u{f5}", true),
    ("ouml", "\u{f6}", true),
    ("ugrave", "\u{f9}", true),
    ("uacute", "\u{fa}", true),
    ("ucirc", "\u{fb}", true),
    ("uuml", "\u{fc}", true),
    ("szlig", "\u{df}", true),
    ("Agrave", "\u{c0}", true),
    ("Aacute", "\u{c1}", true),
    ("Eacute", "\u{c9}", true),
    ("Ntilde", "\u{d1}", true),
    ("Ouml", "\u{d6}", true),
    ("Uuml", "\u{dc}", true),
];

const C1: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

/// Decodes the character reference starting just after an `&` in `s`.
///
/// Returns `None` when `s` does not begin a valid reference, in which case the
/// `&` is literal text. `in_attr` enables the attribute-value restriction on
/// legacy references without a semicolon.
pub fn decode_char_ref(s: &str, in_attr: bool) -> Option<CharRef> {
    let b = s.as_bytes();
    if b.first() == Some(&b'#') {
        decode_numeric(b)
    } else {
        decode_named(s, in_attr)
    }
}

fn decode_numeric(b: &[u8]) -> Option<CharRef> {
    let hex = matches!(b.get(1), Some(b'x') | Some(b'X'));
    let start = if hex { 2 } else { 1 };
    let mut i = start;
    let mut v: u32 = 0;
    while let Some(&c) = b.get(i) {
        let d = if hex {
            (c as char).to_digit(16)
        } else {
            (c as char).to_digit(10)
        };
        let Some(d) = d else { break };
        v = v
            .saturating_mul(if hex { 16 } else { 10 })
            .saturating_add(d)
            .min(0x110000);
        i += 1;
    }
    if i == start {
        return None;
    }
    let mut error = None;
    if b.get(i) == Some(&b';') {
        i += 1;
    } else {
        error = Some("missing-semicolon-after-character-reference");
    }
    let cp = if v == 0 {
        error = Some("null-character-reference");
        0xFFFD
    } else if v > 0x10FFFF {
        error = Some("character-reference-outside-unicode-range");
        0xFFFD
    } else if (0xD800..=0xDFFF).contains(&v) {
        error = Some("surrogate-character-reference");
        0xFFFD
    } else if (0x80..=0x9F).contains(&v) {
        error = Some("control-character-reference");
        C1[(v - 0x80) as usize]
    } else {
        v
    };
    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
    Some(CharRef {
        value: ch.to_string(),
        consumed: i,
        error,
    })
}

fn decode_named(s: &str, in_attr: bool) -> Option<CharRef> {
    let b = s.as_bytes();
    let n = b
        .iter()
        .take(40)
        .take_while(|c| c.is_ascii_alphanumeric())
        .count();
    if n == 0 {
        return None;
    }
    if b.get(n) == Some(&b';') {
        if let Some((_, v, _)) = NAMED.iter().find(|(name, _, _)| *name == &s[..n]) {
            return Some(CharRef {
                value: (*v).to_string(),
                consumed: n + 1,
                error: None,
            });
        }
    }
    // Longest legacy prefix that may omit the semicolon.
    for p in (1..=n).rev() {
        let Some((_, v, _)) = NAMED
            .iter()
            .find(|(name, _, legacy)| *legacy && *name == &s[..p])
        else {
            continue;
        };
        if in_attr {
            let next = b.get(p).copied();
            if next == Some(b'=') || next.is_some_and(|c| c.is_ascii_alphanumeric()) {
                return None;
            }
        }
        return Some(CharRef {
            value: (*v).to_string(),
            consumed: p,
            error: Some("missing-semicolon-after-character-reference"),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Option<(String, usize)> {
        decode_char_ref(s, false).map(|r| (r.value, r.consumed))
    }

    #[test]
    fn named_with_semicolon() {
        assert_eq!(dec("amp;x"), Some(("&".into(), 4)));
        assert_eq!(dec("hellip;"), Some(("\u{2026}".into(), 7)));
    }

    #[test]
    fn legacy_without_semicolon() {
        let r = decode_char_ref("copy 2026", false).unwrap();
        assert_eq!(r.value, "\u{a9}");
        assert_eq!(r.consumed, 4);
        assert_eq!(r.error, Some("missing-semicolon-after-character-reference"));
        // `&ampfoo;` decodes the legacy `&amp` prefix.
        assert_eq!(dec("ampfoo;"), Some(("&".into(), 3)));
    }

    #[test]
    fn non_legacy_requires_semicolon() {
        assert_eq!(dec("hellip x"), None);
    }

    #[test]
    fn attribute_legacy_restriction() {
        assert_eq!(decode_char_ref("amp=1", true), None);
        assert_eq!(decode_char_ref("ampx", true), None);
        assert!(decode_char_ref("amp ", true).is_some());
        assert!(decode_char_ref("amp;", true).is_some());
    }

    #[test]
    fn numeric() {
        assert_eq!(dec("#65;"), Some(("A".into(), 4)));
        assert_eq!(dec("#x41;"), Some(("A".into(), 5)));
        assert_eq!(dec("#X1F600;"), Some(("\u{1F600}".into(), 8)));
        assert_eq!(dec("#65"), Some(("A".into(), 3)));
        assert_eq!(dec("#;"), None);
        assert_eq!(dec("#xg"), None);
    }

    #[test]
    fn numeric_edge_cases() {
        assert_eq!(dec("#0;").unwrap().0, "\u{FFFD}");
        assert_eq!(dec("#xD800;").unwrap().0, "\u{FFFD}");
        assert_eq!(dec("#x110000;").unwrap().0, "\u{FFFD}");
        assert_eq!(dec("#99999999999999999999;").unwrap().0, "\u{FFFD}");
        assert_eq!(dec("#128;").unwrap().0, "\u{20AC}");
        assert_eq!(dec("#x92;").unwrap().0, "\u{2019}");
    }

    #[test]
    fn unknown() {
        assert_eq!(dec("bogus;"), None);
        assert_eq!(dec(" "), None);
        assert_eq!(dec(""), None);
    }
}
