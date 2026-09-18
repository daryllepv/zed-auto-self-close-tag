use ropey::Rope;
use tree_sitter::Tree;

use super::markup;

pub(super) fn allows_self_close(tree: &Tree, text: &Rope, slash_byte: usize) -> bool {
    let Some(tag) = markup::self_closing_tag(tree, slash_byte) else {
        return false;
    };
    if markup::inside_textarea(tag, text) {
        return false;
    }
    // Custom SFC blocks and preprocessed templates are not Vue HTML templates.
    let ancestors = markup::ancestor_start_tags(tag);
    ancestors.first().is_none_or(|root| {
        markup::tag_name(*root, text) == "template"
            && markup::attribute(*root, text, "lang").is_none_or(|lang| lang == "html")
    })
}
