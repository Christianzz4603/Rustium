//! The "in select" and "in select in table" insertion modes.
//!
//! The HTML Living Standard has been revising `<select>` parsing for
//! customizable select elements. The rules here were fitted against the WPT
//! html5lib vectors, not copied from the spec text (the spec page could not be
//! fetched from the build environment): other start and end tags are processed
//! with the in-body rules, and `select` is looked up in the default scope.
//! Each rule is covered by those vectors or by tests in `parser_tests.rs`;
//! treat any remaining difference from the live spec as a known gap.

use crate::token::Token;
use crate::tree_builder::{Mode, TreeBuilder};

impl TreeBuilder {
    /// Whether a `select` is open in the default scope. (Older spec revisions
    /// used a special "select scope"; the current vectors need the default one,
    /// since content like `<b>` may now sit between `select` and the current node.)
    fn select_open(&self) -> bool {
        self.has_in_scope("select", crate::tree_builder::Scope::Default)
    }

    /// Pops through the current `select` and resets the insertion mode.
    fn close_select(&mut self) {
        // Only pop if a select is actually open; `pop_until` would otherwise
        // pop the whole stack (e.g. after `</template>` popped through it).
        if self.open.iter().any(|&n| self.is(n, "select")) {
            self.pop_until("select");
        }
        self.reset_insertion_mode();
    }

    pub(crate) fn in_select(&mut self, tok: Token) -> Option<Token> {
        match tok {
            Token::Character(s) => {
                let s: String = s.chars().filter(|&c| c != '\0').collect();
                if !s.is_empty() {
                    self.insert_text(&s);
                }
                None
            }
            Token::Comment(c) => {
                self.insert_comment(&c, None);
                None
            }
            Token::Doctype { .. } => {
                self.error("unexpected-doctype");
                None
            }
            Token::Eof => self.in_body(tok),
            Token::StartTag {
                ref name,
                ref attrs,
                ..
            } => match name.as_str() {
                "html" => self.in_body(tok),
                "option" => {
                    if self.current_is("option") {
                        self.pop();
                    }
                    self.insert_element("option", attrs);
                    None
                }
                "optgroup" => {
                    if self.current_is("option") {
                        self.pop();
                    }
                    if self.current_is("optgroup") {
                        self.pop();
                    }
                    self.insert_element("optgroup", attrs);
                    None
                }
                "hr" => {
                    if self.current_is("option") {
                        self.pop();
                    }
                    if self.current_is("optgroup") {
                        self.pop();
                    }
                    self.insert_element("hr", attrs);
                    self.pop();
                    None
                }
                "select" => {
                    self.error("unexpected-select-in-select");
                    if self.select_open() {
                        self.close_select();
                    }
                    None
                }
                "input" | "keygen" | "textarea" => {
                    self.error("unexpected-input-in-select");
                    if !self.select_open() {
                        return None;
                    }
                    self.close_select();
                    Some(tok)
                }
                "script" | "template" => self.in_head(tok),
                _ => self.in_body(tok),
            },
            Token::EndTag { ref name } => match name.as_str() {
                "optgroup" => {
                    let n = self.open.len();
                    if self.current_is("option") && n >= 2 && self.is(self.open[n - 2], "optgroup")
                    {
                        self.pop();
                    }
                    if self.current_is("optgroup") {
                        self.pop();
                    } else {
                        self.error("unexpected-end-tag-in-select");
                    }
                    None
                }
                "option" => {
                    if self.current_is("option") {
                        self.pop();
                    } else {
                        self.error("unexpected-end-tag-in-select");
                    }
                    None
                }
                "select" => {
                    if self.select_open() {
                        self.close_select();
                    } else {
                        self.error("unexpected-end-tag-in-select");
                    }
                    None
                }
                "template" => self.in_head(tok),
                _ => self.in_body(tok),
            },
        }
    }

    pub(crate) fn in_select_in_table(&mut self, tok: Token) -> Option<Token> {
        const TABLE_PARTS: &[&str] = &[
            "caption", "table", "tbody", "tfoot", "thead", "tr", "td", "th",
        ];
        match &tok {
            Token::StartTag { name, .. } if TABLE_PARTS.contains(&name.as_str()) => {
                self.error("unexpected-table-part-in-select");
                self.close_select();
                Some(tok)
            }
            Token::EndTag { name } if TABLE_PARTS.contains(&name.as_str()) => {
                self.error("unexpected-table-end-tag-in-select");
                if !self.has_in_scope(name, crate::tree_builder::Scope::Table) {
                    return None;
                }
                self.close_select();
                Some(tok)
            }
            _ => self.in_select(tok),
        }
    }

    /// Mode to use when a `<select>` start tag is inserted from "in body".
    pub(crate) fn mode_after_select_start(&self) -> Mode {
        if matches!(
            self.mode,
            Mode::InTable | Mode::InCaption | Mode::InTableBody | Mode::InRow | Mode::InCell
        ) {
            Mode::InSelectInTable
        } else {
            Mode::InSelect
        }
    }
}
