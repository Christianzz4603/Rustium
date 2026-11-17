//! Arena-backed DOM tree.
//!
//! Nodes live in a single `Vec` and refer to each other by [`NodeId`], which
//! avoids `Rc<RefCell<..>>` overhead and gives a natural handle for future
//! JavaScript bindings. Detached nodes stay in the arena until the `Dom` is
//! dropped (no per-node reclamation yet; see docs/ARCHITECTURE.md).
//!
//! Not yet implemented: event dispatch, live collections, namespaces other
//! than "HTML", shadow DOM.

mod node;
mod query;
mod serialize;
mod tree;

pub use node::{Attribute, ElementData, Node, NodeData, NodeId};
pub use tree::{DomError, Mutation};

use rustium_common::Atom;

/// A document tree plus its node arena.
#[derive(Debug)]
pub struct Dom {
    pub(crate) nodes: Vec<Node>,
    pub(crate) mutations: Vec<Mutation>,
    pub(crate) record_mutations: bool,
    root: NodeId,
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

impl Dom {
    /// Creates a DOM containing only an empty Document node.
    pub fn new() -> Self {
        let doc = Node::new(NodeData::Document);
        Dom {
            nodes: vec![doc],
            mutations: Vec::new(),
            record_mutations: false,
            root: NodeId(0),
        }
    }

    /// Like [`Dom::new`], reserving room for about `nodes` nodes up front.
    pub fn with_capacity(nodes: usize) -> Self {
        let mut dom = Self::new();
        dom.nodes.reserve(nodes);
        dom
    }

    /// The Document node.
    pub fn document(&self) -> NodeId {
        self.root
    }

    /// Number of nodes in the arena (including detached ones).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Always false: a Dom always contains its Document node.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Borrows a node. Panics if `id` came from a different `Dom`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0 as usize]
    }

    fn alloc(&mut self, data: NodeData) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node::new(data));
        id
    }

    /// Creates a detached element with a lowercase local name.
    pub fn create_element(&mut self, name: &str) -> NodeId {
        self.alloc(NodeData::Element(ElementData::new(Atom::new(name))))
    }

    /// Creates a detached text node.
    pub fn create_text(&mut self, text: &str) -> NodeId {
        self.alloc(NodeData::Text(text.to_string()))
    }

    /// Creates a detached comment node.
    pub fn create_comment(&mut self, text: &str) -> NodeId {
        self.alloc(NodeData::Comment(text.to_string()))
    }

    /// Creates a detached doctype node.
    pub fn create_doctype(
        &mut self,
        name: &str,
        public_id: Option<&str>,
        system_id: Option<&str>,
    ) -> NodeId {
        self.alloc(NodeData::Doctype {
            name: name.to_string(),
            public_id: public_id.map(str::to_string),
            system_id: system_id.map(str::to_string),
        })
    }

    /// Creates a detached document fragment.
    pub fn create_fragment(&mut self) -> NodeId {
        self.alloc(NodeData::DocumentFragment)
    }
}
