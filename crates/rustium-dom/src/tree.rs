//! Tree mutation and traversal.

use crate::{Attribute, Dom, NodeData, NodeId};
use rustium_common::Atom;

/// Errors from invalid tree operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomError {
    /// Inserting would make a node its own ancestor.
    HierarchyRequest,
    /// The reference node is not a child of the given parent.
    NotFound,
    /// The parent cannot have children (text, comment, doctype).
    InvalidParent,
}

/// A recorded tree mutation (see [`Dom::set_mutation_recording`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    /// `child` was inserted under `parent`.
    ChildInserted {
        /// New parent.
        parent: NodeId,
        /// Inserted node.
        child: NodeId,
    },
    /// `child` was removed from `parent`.
    ChildRemoved {
        /// Old parent.
        parent: NodeId,
        /// Removed node.
        child: NodeId,
    },
    /// An attribute was set or removed on `element`.
    AttributeChanged {
        /// Target element.
        element: NodeId,
        /// Attribute name.
        name: Atom,
    },
    /// Character data changed.
    TextChanged {
        /// Target node.
        node: NodeId,
    },
}

/// Iterator over a node's children.
pub struct Children<'a> {
    dom: &'a Dom,
    next: Option<NodeId>,
}

impl Iterator for Children<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let cur = self.next?;
        self.next = self.dom.node(cur).next_sibling;
        Some(cur)
    }
}

/// Pre-order (document order) iterator over a subtree, excluding the root.
pub struct Descendants<'a> {
    dom: &'a Dom,
    root: NodeId,
    next: Option<NodeId>,
}

impl Iterator for Descendants<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let cur = self.next?;
        let n = self.dom.node(cur);
        self.next = if let Some(c) = n.first_child {
            Some(c)
        } else {
            let mut at = cur;
            loop {
                if at == self.root {
                    break None;
                }
                let node = self.dom.node(at);
                if let Some(s) = node.next_sibling {
                    break Some(s);
                }
                match node.parent {
                    Some(p) if p != self.root => at = p,
                    _ => break None,
                }
            }
        };
        Some(cur)
    }
}

impl Dom {
    /// Enables or disables mutation recording.
    pub fn set_mutation_recording(&mut self, on: bool) {
        self.record_mutations = on;
    }

    /// Takes and clears recorded mutations.
    pub fn take_mutations(&mut self) -> Vec<Mutation> {
        std::mem::take(&mut self.mutations)
    }

    fn record(&mut self, m: Mutation) {
        if self.record_mutations {
            self.mutations.push(m);
        }
    }

    /// Iterates over the children of `id`.
    pub fn children(&self, id: NodeId) -> Children<'_> {
        Children {
            dom: self,
            next: self.node(id).first_child,
        }
    }

    /// Iterates over all descendants of `id` in document order.
    pub fn descendants(&self, id: NodeId) -> Descendants<'_> {
        Descendants {
            dom: self,
            root: id,
            next: self.node(id).first_child,
        }
    }

    /// Returns true if `ancestor` is `node` or contains it.
    pub fn is_inclusive_ancestor(&self, ancestor: NodeId, node: NodeId) -> bool {
        let mut cur = Some(node);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.node(c).parent;
        }
        false
    }

    fn can_have_children(&self, id: NodeId) -> bool {
        matches!(
            self.node(id).data,
            NodeData::Document | NodeData::DocumentFragment | NodeData::Element(_)
        )
    }

    /// Appends `child` as the last child of `parent`, detaching it first.
    ///
    /// Appending a DocumentFragment moves its children instead.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        self.insert_before(parent, child, None)
    }

    /// Inserts `child` before `reference` (or at the end if `None`).
    pub fn insert_before(
        &mut self,
        parent: NodeId,
        child: NodeId,
        reference: Option<NodeId>,
    ) -> Result<(), DomError> {
        if !self.can_have_children(parent) {
            return Err(DomError::InvalidParent);
        }
        if let Some(r) = reference {
            if self.node(r).parent != Some(parent) {
                return Err(DomError::NotFound);
            }
        }
        if self.is_inclusive_ancestor(child, parent) {
            return Err(DomError::HierarchyRequest);
        }
        if matches!(self.node(child).data, NodeData::DocumentFragment) {
            let kids: Vec<NodeId> = self.children(child).collect();
            for k in kids {
                self.insert_before(parent, k, reference)?;
            }
            return Ok(());
        }
        if reference == Some(child) {
            return Ok(());
        }
        self.detach(child);
        let prev = match reference {
            Some(r) => self.node(r).prev_sibling,
            None => self.node(parent).last_child,
        };
        {
            let n = self.node_mut(child);
            n.parent = Some(parent);
            n.prev_sibling = prev;
            n.next_sibling = reference;
        }
        match prev {
            Some(p) => self.node_mut(p).next_sibling = Some(child),
            None => self.node_mut(parent).first_child = Some(child),
        }
        match reference {
            Some(r) => self.node_mut(r).prev_sibling = Some(child),
            None => self.node_mut(parent).last_child = Some(child),
        }
        self.record(Mutation::ChildInserted { parent, child });
        Ok(())
    }

    /// Removes `child` from `parent`.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> Result<(), DomError> {
        if self.node(child).parent != Some(parent) {
            return Err(DomError::NotFound);
        }
        self.detach(child);
        Ok(())
    }

    /// Detaches a node from its parent, if any.
    pub fn detach(&mut self, id: NodeId) {
        let (parent, prev, next) = {
            let n = self.node(id);
            (n.parent, n.prev_sibling, n.next_sibling)
        };
        let Some(parent) = parent else { return };
        match prev {
            Some(p) => self.node_mut(p).next_sibling = next,
            None => self.node_mut(parent).first_child = next,
        }
        match next {
            Some(n) => self.node_mut(n).prev_sibling = prev,
            None => self.node_mut(parent).last_child = prev,
        }
        let n = self.node_mut(id);
        n.parent = None;
        n.prev_sibling = None;
        n.next_sibling = None;
        self.record(Mutation::ChildRemoved { parent, child: id });
    }

    /// Appends text to `parent`, merging with a trailing text node.
    pub fn append_text(&mut self, parent: NodeId, text: &str) {
        if let Some(last) = self.node(parent).last_child {
            if let NodeData::Text(t) = &mut self.node_mut(last).data {
                t.push_str(text);
                self.record(Mutation::TextChanged { node: last });
                return;
            }
        }
        let t = self.create_text(text);
        let _ = self.append_child(parent, t);
    }

    /// Inserts text into `parent` before `reference` (or at the end if `None`),
    /// merging with an adjacent preceding text node.
    pub fn insert_text_before(&mut self, parent: NodeId, reference: Option<NodeId>, text: &str) {
        let Some(r) = reference else {
            self.append_text(parent, text);
            return;
        };
        if let Some(prev) = self.node(r).prev_sibling {
            if let NodeData::Text(t) = &mut self.node_mut(prev).data {
                t.push_str(text);
                self.record(Mutation::TextChanged { node: prev });
                return;
            }
        }
        let t = self.create_text(text);
        let _ = self.insert_before(parent, t, Some(r));
    }

    /// Gets an attribute value from an element.
    pub fn get_attribute(&self, el: NodeId, name: &str) -> Option<&str> {
        self.node(el).as_element()?.attr(name)
    }

    /// Sets an attribute, replacing any existing value. No-op on non-elements.
    pub fn set_attribute(&mut self, el: NodeId, name: &str, value: &str) {
        let NodeData::Element(e) = &mut self.node_mut(el).data else {
            return;
        };
        if let Some(a) = e.attrs.iter_mut().find(|a| a.name == name) {
            a.value = value.to_string();
        } else {
            e.attrs.push(Attribute {
                name: Atom::new(name),
                value: value.to_string(),
            });
        }
        self.record(Mutation::AttributeChanged {
            element: el,
            name: Atom::new(name),
        });
    }

    /// Sets an attribute only if it is not already present (used by the parser).
    pub fn set_attribute_if_absent(&mut self, el: NodeId, name: &str, value: &str) {
        if self.get_attribute(el, name).is_none() {
            self.set_attribute(el, name, value);
        }
    }

    /// Removes an attribute. Returns whether it existed.
    pub fn remove_attribute(&mut self, el: NodeId, name: &str) -> bool {
        let NodeData::Element(e) = &mut self.node_mut(el).data else {
            return false;
        };
        let before = e.attrs.len();
        e.attrs.retain(|a| a.name != name);
        let removed = e.attrs.len() != before;
        if removed {
            self.record(Mutation::AttributeChanged {
                element: el,
                name: Atom::new(name),
            });
        }
        removed
    }
}
