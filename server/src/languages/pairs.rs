use ropey::Rope;
use tree_sitter::Tree;

use super::{Language, markup};

pub(super) fn empty_pair_end(
    language: Language,
    tree: &Tree,
    text: &Rope,
    delimiter_byte: usize,
) -> Option<usize> {
    let (opening, closing, name) = match language {
        Language::Tsx | Language::Jsx => ("jsx_opening_element", "jsx_closing_element", None),
        Language::Xml => ("STag", "ETag", Some("Name")),
        _ => ("start_tag", "end_tag", Some("tag_name")),
    };
    let delimiter = tree
        .root_node()
        .descendant_for_byte_range(delimiter_byte, delimiter_byte + 1)?;
    if delimiter.kind() != ">" || delimiter.byte_range() != (delimiter_byte..delimiter_byte + 1) {
        return None;
    }
    let start = delimiter.parent()?;
    if start.kind() != opening || start.has_error() {
        return None;
    }
    let end = start.next_named_sibling()?;
    if end.kind() != closing || end.has_error() || start.end_byte() != end.start_byte() {
        return None;
    }
    let (start_name, end_name) = match name {
        Some(kind) => (markup::child(start, kind)?, markup::child(end, kind)?),
        None => (
            start.child_by_field_name("name")?,
            end.child_by_field_name("name")?,
        ),
    };
    (text.byte_slice(start_name.byte_range()) == text.byte_slice(end_name.byte_range()))
        .then_some(end.end_byte())
}
