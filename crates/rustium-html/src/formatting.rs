//! Active formatting elements and the adoption agency algorithm.
//!
//! This is what recovers sensible trees from misnested markup such as
//! `<b><i></b></i>` or `<a>1<div>2</a>3</div>`. Foster parenting is not
//! implemented, so the "appropriate place for inserting" is always the common
//! ancestor itself.

use crate::tree_builder::{is_special, Fmt, TreeBuilder, MAX_OPEN_DEPTH};
use rustium_dom::NodeId;

/// Upper bound on the active formatting list; the spec has none, but an
/// unbounded list makes reconstruction quadratic on hostile input.
const MAX_FORMATTING_ENTRIES: usize = 256;

impl TreeBuilder {
    /// Pushes onto the list of active formatting elements, applying the
    /// "Noah's Ark" clause (at most three identical entries since the last
    /// marker) and a hard size cap that bounds reconstruction work.
    pub fn push_formatting(&mut self, id: NodeId, name: &str, attrs: &[(String, String)]) {
        let mut sorted = attrs.to_vec();
        sorted.sort();
        let mut matches: Vec<usize> = Vec::new();
        for (i, f) in self.formatting.iter().enumerate().rev() {
            match f {
                Fmt::Marker => break,
                Fmt::Element {
                    name: n, attrs: a, ..
                } if n == name => {
                    let mut other = a.clone();
                    other.sort();
                    if other == sorted {
                        matches.push(i);
                    }
                }
                _ => {}
            }
        }
        if matches.len() >= 3 {
            // `matches` is ordered latest-first, so the last element is the earliest.
            self.formatting.remove(*matches.last().unwrap());
        }
        if self.formatting.len() >= MAX_FORMATTING_ENTRIES {
            if let Some(i) = self
                .formatting
                .iter()
                .position(|f| matches!(f, Fmt::Element { .. }))
            {
                self.formatting.remove(i);
                self.unsupported("formatting-list-limit-exceeded");
            }
        }
        self.formatting.push(Fmt::Element {
            id,
            name: name.to_string(),
            attrs: attrs.to_vec(),
        });
    }

    pub fn push_marker(&mut self) {
        self.formatting.push(Fmt::Marker);
    }

    pub fn clear_formatting_to_marker(&mut self) {
        while let Some(f) = self.formatting.pop() {
            if matches!(f, Fmt::Marker) {
                break;
            }
        }
    }

    fn fmt_ok(&self, idx: usize) -> bool {
        match &self.formatting[idx] {
            Fmt::Marker => true,
            Fmt::Element { id, .. } => self.open.contains(id),
        }
    }

    pub fn reconstruct_formatting(&mut self) {
        let len = self.formatting.len();
        if len == 0 || self.fmt_ok(len - 1) {
            return;
        }
        // Past the depth cap new elements never join the stack, so
        // reconstruction would re-create the whole list on every token.
        if self.open.len() >= MAX_OPEN_DEPTH {
            return;
        }
        let mut i = len - 1;
        while i > 0 && !self.fmt_ok(i - 1) {
            i -= 1;
        }
        for j in i..len {
            let Fmt::Element { name, attrs, .. } = self.formatting[j].clone() else {
                continue;
            };
            let id = self.insert_element(&name, &attrs);
            self.formatting[j] = Fmt::Element { id, name, attrs };
        }
    }

    /// Index of the last formatting entry named `name` after the last marker.
    pub fn find_formatting(&self, name: &str) -> Option<usize> {
        for (i, f) in self.formatting.iter().enumerate().rev() {
            match f {
                Fmt::Marker => return None,
                Fmt::Element { name: n, .. } if n == name => return Some(i),
                _ => {}
            }
        }
        None
    }

    fn formatting_index_of(&self, node: NodeId) -> Option<usize> {
        self.formatting
            .iter()
            .position(|f| matches!(f, Fmt::Element { id, .. } if *id == node))
    }

    /// Runs the adoption agency algorithm for an end tag (or implied close) of
    /// `subject`. Returns false if the caller should fall back to the "any other
    /// end tag" steps.
    pub fn adoption_agency(&mut self, subject: &str) -> bool {
        if let Some(&cur) = self.open.last() {
            if self.is(cur, subject) && self.formatting_index_of(cur).is_none() {
                self.open.pop();
                return true;
            }
        }
        for _ in 0..8 {
            let Some(fi) = self.find_formatting(subject) else {
                return false;
            };
            let Fmt::Element {
                id: fe,
                name: fe_name,
                attrs: fe_attrs,
            } = self.formatting[fi].clone()
            else {
                return false;
            };
            let Some(fe_pos) = self.open.iter().position(|&n| n == fe) else {
                self.error("formatting-element-not-open");
                self.formatting.remove(fi);
                return true;
            };
            if !self.node_in_scope(fe) {
                self.error("formatting-element-not-in-scope");
                return true;
            }
            if self.open.last() != Some(&fe) {
                self.error("formatting-element-not-current");
            }
            let fb_pos = self.open[fe_pos + 1..]
                .iter()
                .position(|&n| is_special(self.name(n).as_str()))
                .map(|p| p + fe_pos + 1);
            let Some(fb_pos) = fb_pos else {
                self.open.truncate(fe_pos);
                self.formatting.remove(fi);
                return true;
            };
            if fe_pos == 0 {
                return true;
            }
            let furthest_block = self.open[fb_pos];
            let common_ancestor = self.open[fe_pos - 1];
            let mut bookmark = fi;
            let mut last_node = furthest_block;
            let mut node_pos = fb_pos;
            let mut inner = 0;
            loop {
                inner += 1;
                if node_pos == 0 {
                    break;
                }
                node_pos -= 1;
                let node = self.open[node_pos];
                if node == fe {
                    break;
                }
                let mut nf = self.formatting_index_of(node);
                if inner > 3 {
                    if let Some(idx) = nf {
                        self.formatting.remove(idx);
                        if idx < bookmark {
                            bookmark -= 1;
                        }
                        nf = None;
                    }
                }
                let Some(idx) = nf else {
                    self.open.remove(node_pos);
                    continue;
                };
                let Fmt::Element { name, attrs, .. } = self.formatting[idx].clone() else {
                    continue;
                };
                let new = self.create_element(&name, &attrs);
                self.formatting[idx] = Fmt::Element {
                    id: new,
                    name,
                    attrs,
                };
                self.open[node_pos] = new;
                if last_node == furthest_block {
                    bookmark = idx + 1;
                }
                let _ = self.dom.append_child(new, last_node);
                last_node = new;
            }
            let _ = self.dom.append_child(common_ancestor, last_node);
            let new_fe = self.create_element(&fe_name, &fe_attrs);
            let kids: Vec<NodeId> = self.dom.children(furthest_block).collect();
            for k in kids {
                let _ = self.dom.append_child(new_fe, k);
            }
            let _ = self.dom.append_child(furthest_block, new_fe);
            if let Some(old) = self.formatting_index_of(fe) {
                self.formatting.remove(old);
                if old < bookmark {
                    bookmark -= 1;
                }
            }
            let bookmark = bookmark.min(self.formatting.len());
            self.formatting.insert(
                bookmark,
                Fmt::Element {
                    id: new_fe,
                    name: fe_name,
                    attrs: fe_attrs,
                },
            );
            if let Some(p) = self.open.iter().position(|&n| n == fe) {
                self.open.remove(p);
            }
            let fb_now = self
                .open
                .iter()
                .position(|&n| n == furthest_block)
                .unwrap_or(0);
            self.open.insert(fb_now + 1, new_fe);
        }
        true
    }

    /// "Any other end tag" steps of the in-body insertion mode.
    pub fn any_other_end_tag(&mut self, subject: &str) {
        for i in (0..self.open.len()).rev() {
            let node = self.open[i];
            if self.is(node, subject) {
                self.generate_implied_end_tags(Some(subject));
                if self.open.last() != Some(&node) {
                    self.error("unclosed-element-at-end-tag");
                }
                self.open.truncate(i);
                return;
            }
            if is_special(self.name(node).as_str()) {
                self.error("unexpected-end-tag");
                return;
            }
        }
    }
}
