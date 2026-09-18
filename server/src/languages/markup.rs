use ropey::Rope;
use tree_sitter::{Node, Tree};

// Astro, Svelte, Vue, and HTML grammars share these HTML node definitions.
pub(super) fn self_closing_tag(tree: &Tree, slash_byte: usize) -> Option<Node<'_>> {
    tree.root_node()
        .descendant_for_byte_range(slash_byte, slash_byte + 2)
        .filter(|node| node.kind() == "/>" && node.byte_range() == (slash_byte..slash_byte + 2))
        .and_then(|node| node.parent())
        .filter(|tag| tag.kind() == "self_closing_tag" && !tag.has_error())
        .filter(|tag| child(*tag, "tag_name").is_some())
        .filter(|tag| !has_error_before(*tag))
}

// Markup grammars can recover broken comments, declarations, or interpolations
// as ordinary tags. A locally valid delimiter is unsafe after such a recovery.
pub(super) fn has_error_before(mut node: Node<'_>) -> bool {
    loop {
        let Some(parent) = node.parent() else {
            return false;
        };
        if parent.is_error() {
            return true;
        }
        if !parent.has_error() {
            node = parent;
            continue;
        }
        let mut previous = node.prev_sibling();
        while let Some(sibling) = previous {
            if sibling.has_error() {
                return true;
            }
            previous = sibling.prev_sibling();
        }
        node = parent;
    }
}

pub(super) fn child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    node.named_children(&mut node.walk())
        .find(|node| node.kind() == kind)
}

pub(super) fn tag_name(tag: Node<'_>, text: &Rope) -> String {
    child(tag, "tag_name")
        .map(|name| text.byte_slice(name.byte_range()).to_string())
        .unwrap_or_default()
}

pub(super) fn attribute(tag: Node<'_>, text: &Rope, name: &str) -> Option<String> {
    tag.named_children(&mut tag.walk())
        .filter(|node| node.kind() == "attribute")
        .find_map(|attr| {
            let key = child(attr, "attribute_name")?;
            if !text
                .byte_slice(key.byte_range())
                .to_string()
                .eq_ignore_ascii_case(name)
            {
                return None;
            }
            let value =
                child(attr, "attribute_value").or_else(|| child(attr, "quoted_attribute_value"))?;
            Some(
                text.byte_slice(value.byte_range())
                    .to_string()
                    .trim_matches(['\'', '"'])
                    .to_owned(),
            )
        })
}

pub(super) fn ancestor_start_tags(tag: Node<'_>) -> Vec<Node<'_>> {
    let mut tags = Vec::new();
    let mut parent = tag.parent();
    while let Some(node) = parent {
        if let Some(start) = child(node, "start_tag") {
            tags.push(start);
        }
        parent = node.parent();
    }
    tags.reverse();
    tags
}

pub(super) fn inside_textarea(tag: Node<'_>, text: &Rope) -> bool {
    // The template grammars inherit HTML's node structure, but do not treat
    // textarea content as text. Capitalized component names remain distinct.
    ancestor_start_tags(tag)
        .iter()
        .any(|start| tag_name(*start, text) == "textarea")
}
