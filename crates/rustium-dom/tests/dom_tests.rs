#![allow(missing_docs)]
use rustium_dom::{Dom, DomError, Mutation, NodeData};

fn names(dom: &Dom, ids: impl Iterator<Item = rustium_dom::NodeId>) -> Vec<String> {
    ids.filter_map(|i| dom.node(i).as_element().map(|e| e.name.to_string()))
        .collect()
}

#[test]
fn append_and_children_order() {
    let mut dom = Dom::new();
    let root = dom.document();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    let c = dom.create_element("c");
    dom.append_child(root, a).unwrap();
    dom.append_child(root, b).unwrap();
    dom.append_child(root, c).unwrap();
    assert_eq!(names(&dom, dom.children(root)), ["a", "b", "c"]);
    assert_eq!(dom.node(b).parent, Some(root));
    assert_eq!(dom.node(b).prev_sibling, Some(a));
    assert_eq!(dom.node(b).next_sibling, Some(c));
}

#[test]
fn insert_before_and_move() {
    let mut dom = Dom::new();
    let root = dom.document();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    let c = dom.create_element("c");
    dom.append_child(root, a).unwrap();
    dom.append_child(root, c).unwrap();
    dom.insert_before(root, b, Some(c)).unwrap();
    assert_eq!(names(&dom, dom.children(root)), ["a", "b", "c"]);
    // Moving an attached node re-parents it.
    dom.insert_before(root, c, Some(a)).unwrap();
    assert_eq!(names(&dom, dom.children(root)), ["c", "a", "b"]);
}

#[test]
fn remove_child_updates_links() {
    let mut dom = Dom::new();
    let root = dom.document();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    dom.append_child(root, a).unwrap();
    dom.append_child(root, b).unwrap();
    dom.remove_child(root, a).unwrap();
    assert_eq!(names(&dom, dom.children(root)), ["b"]);
    assert_eq!(dom.node(a).parent, None);
    assert_eq!(dom.node(b).prev_sibling, None);
    assert_eq!(dom.remove_child(root, a), Err(DomError::NotFound));
}

#[test]
fn cycles_and_invalid_parents_rejected() {
    let mut dom = Dom::new();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    dom.append_child(a, b).unwrap();
    assert_eq!(dom.append_child(b, a), Err(DomError::HierarchyRequest));
    assert_eq!(dom.append_child(a, a), Err(DomError::HierarchyRequest));
    let t = dom.create_text("x");
    let c = dom.create_element("c");
    assert_eq!(dom.append_child(t, c), Err(DomError::InvalidParent));
}

#[test]
fn fragment_children_are_moved() {
    let mut dom = Dom::new();
    let root = dom.document();
    let frag = dom.create_fragment();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    dom.append_child(frag, a).unwrap();
    dom.append_child(frag, b).unwrap();
    dom.append_child(root, frag).unwrap();
    assert_eq!(names(&dom, dom.children(root)), ["a", "b"]);
    assert!(dom.node(frag).first_child.is_none());
}

#[test]
fn descendants_are_document_order() {
    let mut dom = Dom::new();
    let root = dom.document();
    let a = dom.create_element("a");
    let b = dom.create_element("b");
    let c = dom.create_element("c");
    let d = dom.create_element("d");
    dom.append_child(root, a).unwrap();
    dom.append_child(a, b).unwrap();
    dom.append_child(b, c).unwrap();
    dom.append_child(a, d).unwrap();
    assert_eq!(names(&dom, dom.descendants(root)), ["a", "b", "c", "d"]);
    assert_eq!(names(&dom, dom.descendants(a)), ["b", "c", "d"]);
    assert_eq!(names(&dom, dom.descendants(c)), Vec::<String>::new());
}

#[test]
fn attributes() {
    let mut dom = Dom::new();
    let e = dom.create_element("div");
    dom.set_attribute(e, "id", "x");
    dom.set_attribute(e, "class", "a b");
    dom.set_attribute(e, "id", "y");
    assert_eq!(dom.get_attribute(e, "id"), Some("y"));
    dom.set_attribute_if_absent(e, "id", "z");
    assert_eq!(dom.get_attribute(e, "id"), Some("y"));
    assert!(dom.remove_attribute(e, "class"));
    assert!(!dom.remove_attribute(e, "class"));
    assert_eq!(dom.get_attribute(e, "class"), None);
}

#[test]
fn queries_and_text_content() {
    let mut dom = Dom::new();
    let root = dom.document();
    let div = dom.create_element("div");
    let p = dom.create_element("p");
    dom.set_attribute(p, "id", "target");
    dom.append_child(root, div).unwrap();
    dom.append_child(div, p).unwrap();
    dom.append_text(p, "hello ");
    dom.append_text(p, "world");
    assert_eq!(dom.get_element_by_id(root, "target"), Some(p));
    assert_eq!(dom.get_element_by_id(root, "nope"), None);
    assert_eq!(dom.get_elements_by_tag_name(root, "p"), vec![p]);
    assert_eq!(dom.text_content(div), "hello world");
    // Adjacent appended text merges into one node.
    assert_eq!(dom.children(p).count(), 1);
}

#[test]
fn mutation_recording() {
    let mut dom = Dom::new();
    let root = dom.document();
    dom.set_mutation_recording(true);
    let a = dom.create_element("a");
    dom.append_child(root, a).unwrap();
    dom.set_attribute(a, "k", "v");
    dom.remove_child(root, a).unwrap();
    let m = dom.take_mutations();
    assert_eq!(m.len(), 3);
    assert!(matches!(m[0], Mutation::ChildInserted { .. }));
    assert!(matches!(m[1], Mutation::AttributeChanged { .. }));
    assert!(matches!(m[2], Mutation::ChildRemoved { .. }));
    assert!(dom.take_mutations().is_empty());
}

#[test]
fn serialization_escapes_and_voids() {
    let mut dom = Dom::new();
    let root = dom.document();
    let p = dom.create_element("p");
    dom.set_attribute(p, "title", "a\"b&c");
    dom.append_child(root, p).unwrap();
    dom.append_text(p, "1 < 2 & 3");
    let br = dom.create_element("br");
    dom.append_child(p, br).unwrap();
    assert_eq!(
        dom.inner_html(root),
        "<p title=\"a&quot;b&amp;c\">1 &lt; 2 &amp; 3<br></p>"
    );
    assert!(matches!(dom.node(root).data, NodeData::Document));
}
