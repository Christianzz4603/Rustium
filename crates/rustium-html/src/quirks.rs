//! Quirks-mode detection from a DOCTYPE token (HTML Living Standard, "initial"
//! insertion mode).
//!
//! The spec has three modes: no-quirks, limited-quirks, and quirks. The parser
//! itself only behaves differently in full quirks mode (`<table>` does not
//! close an open `<p>`), so only that distinction is computed. Limited-quirks
//! mode affects layout, which does not exist yet.

/// Public identifier prefixes that force quirks mode (compared ASCII
/// case-insensitively).
const QUIRKY_PUBLIC_PREFIXES: &[&str] = &[
    "+//silmaril//dtd html pro v0r11 19970101//",
    "-//as//dtd html 3.0 aswedit + extensions//",
    "-//advasoft ltd//dtd html 3.0 aswedit + extensions//",
    "-//ietf//dtd html 2.0 level 1//",
    "-//ietf//dtd html 2.0 level 2//",
    "-//ietf//dtd html 2.0 strict level 1//",
    "-//ietf//dtd html 2.0 strict level 2//",
    "-//ietf//dtd html 2.0 strict//",
    "-//ietf//dtd html 2.0//",
    "-//ietf//dtd html 2.1e//",
    "-//ietf//dtd html 3.0//",
    "-//ietf//dtd html 3.2 final//",
    "-//ietf//dtd html 3.2//",
    "-//ietf//dtd html 3//",
    "-//ietf//dtd html level 0//",
    "-//ietf//dtd html level 1//",
    "-//ietf//dtd html level 2//",
    "-//ietf//dtd html level 3//",
    "-//ietf//dtd html strict level 0//",
    "-//ietf//dtd html strict level 1//",
    "-//ietf//dtd html strict level 2//",
    "-//ietf//dtd html strict level 3//",
    "-//ietf//dtd html strict//",
    "-//ietf//dtd html//",
    "-//metrius//dtd metrius presentational//",
    "-//microsoft//dtd internet explorer 2.0 html strict//",
    "-//microsoft//dtd internet explorer 2.0 html//",
    "-//microsoft//dtd internet explorer 2.0 tables//",
    "-//microsoft//dtd internet explorer 3.0 html strict//",
    "-//microsoft//dtd internet explorer 3.0 html//",
    "-//microsoft//dtd internet explorer 3.0 tables//",
    "-//netscape comm. corp.//dtd html//",
    "-//netscape comm. corp.//dtd strict html//",
    "-//o'reilly and associates//dtd html 2.0//",
    "-//o'reilly and associates//dtd html extended 1.0//",
    "-//o'reilly and associates//dtd html extended relaxed 1.0//",
    "-//softquad software//dtd hotmetal pro 6.0::19990601::extensions to html 4.0//",
    "-//softquad//dtd hotmetal pro 4.0::19971010::extensions to html 4.0//",
    "-//spyglass//dtd html 2.0 extended//",
    "-//sq//dtd html 2.0 hotmetal + extensions//",
    "-//sun microsystems corp.//dtd hotjava html//",
    "-//sun microsystems corp.//dtd hotjava strict html//",
    "-//w3c//dtd html 3 1995-03-24//",
    "-//w3c//dtd html 3.2 draft//",
    "-//w3c//dtd html 3.2 final//",
    "-//w3c//dtd html 3.2//",
    "-//w3c//dtd html 3.2s draft//",
    "-//w3c//dtd html 4.0 frameset//",
    "-//w3c//dtd html 4.0 transitional//",
    "-//w3c//dtd html experimental 19960712//",
    "-//w3c//dtd html experimental 970421//",
    "-//w3c//dtd w3 html//",
    "-//w3o//dtd w3 html 3.0//",
    "-//webtechs//dtd mozilla html 2.0//",
    "-//webtechs//dtd mozilla html//",
];

/// Public identifiers that force quirks mode when they match exactly.
const QUIRKY_PUBLIC_EXACT: &[&str] = &[
    "-//w3o//dtd w3 html strict 3.0//en//",
    "-/w3c/dtd html 4.0 transitional/en",
    "html",
];

const QUIRKY_SYSTEM_EXACT: &str = "http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd";

/// Prefixes that force quirks mode only when there is no system identifier.
const QUIRKY_IF_NO_SYSTEM: &[&str] = &[
    "-//w3c//dtd html 4.01 frameset//",
    "-//w3c//dtd html 4.01 transitional//",
];

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// Returns true if a DOCTYPE with these parts puts the document in quirks mode.
pub(crate) fn is_quirks_mode(
    name: Option<&str>,
    public_id: Option<&str>,
    system_id: Option<&str>,
    force_quirks: bool,
) -> bool {
    if force_quirks || name != Some("html") {
        return true;
    }
    if let Some(p) = public_id {
        if QUIRKY_PUBLIC_EXACT
            .iter()
            .any(|q| p.eq_ignore_ascii_case(q))
        {
            return true;
        }
        if QUIRKY_PUBLIC_PREFIXES.iter().any(|q| starts_with_ci(p, q)) {
            return true;
        }
        if system_id.is_none() && QUIRKY_IF_NO_SYSTEM.iter().any(|q| starts_with_ci(p, q)) {
            return true;
        }
    }
    system_id.is_some_and(|s| s.eq_ignore_ascii_case(QUIRKY_SYSTEM_EXACT))
}

#[cfg(test)]
mod tests {
    use super::is_quirks_mode;

    #[test]
    fn plain_html_doctype_is_no_quirks() {
        assert!(!is_quirks_mode(Some("html"), None, None, false));
    }

    #[test]
    fn missing_or_wrong_name_or_force_flag_is_quirks() {
        assert!(is_quirks_mode(None, None, None, false));
        assert!(is_quirks_mode(Some("potato"), None, None, false));
        assert!(is_quirks_mode(Some("html"), None, None, true));
    }

    #[test]
    fn legacy_public_identifiers_are_quirks() {
        assert!(is_quirks_mode(
            Some("html"),
            Some("-//W3C//DTD HTML 3.2 Final//EN"),
            None,
            false
        ));
        assert!(is_quirks_mode(Some("html"), Some("HTML"), None, false));
    }

    #[test]
    fn html401_transitional_depends_on_system_identifier() {
        let p = Some("-//W3C//DTD HTML 4.01 Transitional//EN");
        assert!(is_quirks_mode(Some("html"), p, None, false));
        assert!(!is_quirks_mode(
            Some("html"),
            p,
            Some("http://www.w3.org/TR/html4/loose.dtd"),
            false
        ));
    }

    #[test]
    fn xhtml_1_0_is_not_full_quirks() {
        assert!(!is_quirks_mode(
            Some("html"),
            Some("-//W3C//DTD XHTML 1.0 Transitional//EN"),
            Some("http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd"),
            false
        ));
    }
}
