//! Serialization: an HTML-ish serializer and a stable debug tree dump used by tests.

use crate::{Dom, NodeData, NodeId};

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

impl Dom {
    /// Serializes the children of `id` as HTML (not the node itself).
    pub fn inner_html(&self, id: NodeId) -> String {
        let mut out = String::new();
        for c in self.children(id) {
            self.write_html(c, &mut out);
        }
        out
    }

    fn write_html(&self, id: NodeId, out: &mut String) {
        match &self.node(id).data {
            NodeData::Text(t) => {
                // Raw-text elements are not special-cased yet.
                escape_text(t, out)
            }
            NodeData::Comment(c) => {
                out.push_str("<!--");
                out.push_str(c);
                out.push_str("-->");
            }
            NodeData::Doctype { name } => {
                out.push_str("<!DOCTYPE ");
                out.push_str(name);
                out.push('>');
            }
            NodeData::Element(e) => {
                out.push('<');
                out.push_str(e.name.as_str());
                for a in &e.attrs {
                    out.push(' ');
                    out.push_str(a.name.as_str());
                    out.push_str("=\"");
                    escape_attr(&a.value, out);
                    out.push('"');
                }
                out.push('>');
                if !VOID.contains(&e.name.as_str()) {
                    for c in self.children(id) {
                        self.write_html(c, out);
                    }
                    out.push_str("</");
                    out.push_str(e.name.as_str());
                    out.push('>');
                }
            }
            NodeData::Document | NodeData::DocumentFragment => {
                for c in self.children(id) {
                    self.write_html(c, out);
                }
            }
        }
    }

    /// Indented tree dump, one node per line, in the style of html5lib test expectations.
    pub fn dump_tree(&self, id: NodeId) -> String {
        let mut out = String::new();
        for c in self.children(id) {
            self.dump_node(c, 0, &mut out);
        }
        out
    }

    fn dump_node(&self, id: NodeId, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        match &self.node(id).data {
            NodeData::Doctype { name } => out.push_str(&format!("{pad}<!DOCTYPE {name}>\n")),
            NodeData::Comment(c) => out.push_str(&format!("{pad}<!-- {c} -->\n")),
            NodeData::Text(t) => out.push_str(&format!("{pad}\"{t}\"\n")),
            NodeData::Element(e) => {
                out.push_str(&format!("{pad}<{}>\n", e.name));
                for a in &e.attrs {
                    out.push_str(&format!("{pad}  {}=\"{}\"\n", a.name, a.value));
                }
                for c in self.children(id) {
                    self.dump_node(c, depth + 1, out);
                }
            }
            NodeData::Document | NodeData::DocumentFragment => {
                for c in self.children(id) {
                    self.dump_node(c, depth, out);
                }
            }
        }
    }
}
