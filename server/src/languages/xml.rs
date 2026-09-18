use ropey::Rope;
use tree_sitter::Tree;

use super::markup;

pub(super) fn allows_self_close(tree: &Tree, text: &Rope, slash_byte: usize) -> bool {
    tree.root_node()
        .descendant_for_byte_range(slash_byte, slash_byte + 2)
        .filter(|node| node.kind() == "/>" && node.byte_range() == (slash_byte..slash_byte + 2))
        .and_then(|node| node.parent())
        .is_some_and(|tag| {
            tag.kind() == "EmptyElemTag"
                && !tag.has_error()
                && !markup::has_error_before(tag)
                // The external scanner accepts digits as the first character
                // of a tag name, although XML does not.
                && markup::child(tag, "Name").is_some_and(|name| {
                    text.byte_slice(name.byte_range()).chars().next()
                        .is_some_and(|ch| ch.is_alphabetic() || matches!(ch, '_' | ':'))
                })
        })
}
