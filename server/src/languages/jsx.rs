use tree_sitter::Tree;

pub(super) fn allows_self_close(tree: &Tree, slash_byte: usize) -> bool {
    tree.root_node()
        .descendant_for_byte_range(slash_byte, slash_byte + 2)
        .filter(|delimiter| {
            delimiter.kind() == "/>" && delimiter.byte_range() == (slash_byte..slash_byte + 2)
        })
        .and_then(|delimiter| delimiter.parent())
        .is_some_and(|element| {
            element.kind() == "jsx_self_closing_element"
                && !element.has_error()
                && element.child_by_field_name("name").is_some()
        })
}
