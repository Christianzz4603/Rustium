//! Node types.

use rustium_common::Atom;

/// Handle to a node within one [`crate::Dom`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    /// Raw arena index.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An element attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Attribute name (lowercase for HTML).
    pub name: Atom,
    /// Attribute value.
    pub value: String,
}

/// Element-specific data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementData {
    /// Local tag name (lowercase for HTML).
    pub name: Atom,
    /// Attributes in source order, names unique.
    pub attrs: Vec<Attribute>,
}

impl ElementData {
    pub(crate) fn new(name: Atom) -> Self {
        ElementData {
            name,
            attrs: Vec::new(),
        }
    }

    /// Returns an attribute value by name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }
}

/// Payload of a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeData {
    /// The document root.
    Document,
    /// A document fragment.
    DocumentFragment,
    /// `<!DOCTYPE name>`.
    Doctype {
        /// Doctype name.
        name: String,
    },
    /// An element.
    Element(ElementData),
    /// Character data.
    Text(String),
    /// `<!-- comment -->`.
    Comment(String),
}

/// A node with its tree links.
#[derive(Debug, Clone)]
pub struct Node {
    /// Parent, if attached.
    pub parent: Option<NodeId>,
    /// First child.
    pub first_child: Option<NodeId>,
    /// Last child.
    pub last_child: Option<NodeId>,
    /// Previous sibling.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling.
    pub next_sibling: Option<NodeId>,
    /// Node payload.
    pub data: NodeData,
}

impl Node {
    pub(crate) fn new(data: NodeData) -> Self {
        Node {
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data,
        }
    }

    /// Element data if this is an element.
    pub fn as_element(&self) -> Option<&ElementData> {
        match &self.data {
            NodeData::Element(e) => Some(e),
            _ => None,
        }
    }
}
