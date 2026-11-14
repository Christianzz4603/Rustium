//! Simple DOM queries. Full selector-based querying arrives with rustium-css.

use crate::{Dom, NodeData, NodeId};

impl Dom {
    /// First element in document order under `root` whose `id` attribute equals `id`.
    pub fn get_element_by_id(&self, root: NodeId, id: &str) -> Option<NodeId> {
        self.descendants(root)
            .find(|&n| self.get_attribute(n, "id") == Some(id))
    }

    /// All elements under `root` with the given lowercase tag name, in document order.
    pub fn get_elements_by_tag_name(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        self.descendants(root)
            .filter(|&n| matches!(&self.node(n).data, NodeData::Element(e) if e.name == tag))
            .collect()
    }

    /// Concatenated text of all text nodes under `root`.
    pub fn text_content(&self, root: NodeId) -> String {
        let mut out = String::new();
        if let NodeData::Text(t) = &self.node(root).data {
            out.push_str(t);
        }
        for n in self.descendants(root) {
            if let NodeData::Text(t) = &self.node(n).data {
                out.push_str(t);
            }
        }
        out
    }
}
