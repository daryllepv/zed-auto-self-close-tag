use ropey::Rope;
use tree_sitter::Tree;

use super::{html, markup};

pub(super) fn allows_self_close(tree: &Tree, text: &Rope, slash_byte: usize) -> bool {
    let Some(tag) = markup::self_closing_tag(tree, slash_byte) else {
        return false;
    };
    if markup::inside_textarea(tag, text) {
        return false;
    }
    let name = markup::tag_name(tag, text);
    name.starts_with(char::is_uppercase)
        || name.contains('.')
        || matches!(
            name.as_str(),
            "slot"
                | "svelte:component"
                | "svelte:self"
                | "svelte:element"
                | "svelte:window"
                | "svelte:document"
                | "svelte:body"
                | "svelte:options"
                | "svelte:head"
                | "svelte:boundary"
                | "svelte:fragment"
        )
        || html::tag_may_self_close(tag, text)
}
