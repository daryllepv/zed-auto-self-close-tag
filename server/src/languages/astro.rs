use ropey::Rope;
use tree_sitter::{Parser, Tree};

use crate::{Error, parse};

use super::{jsx, markup};

pub(super) fn allows_self_close(
    tree: &Tree,
    text: &Rope,
    slash_byte: usize,
) -> Result<bool, Error> {
    let Some(tag) = markup::self_closing_tag(tree, slash_byte) else {
        return Ok(false);
    };
    if markup::inside_textarea(tag, text) {
        return Ok(false);
    }
    let mut parent = tag.parent();
    let mut expression = None;
    while let Some(node) = parent {
        if node.kind() == "html_interpolation" {
            expression = Some(node);
        }
        parent = node.parent();
    }
    if let Some(expression) = expression {
        // Astro's outer grammar also recognizes tag-like text inside JS strings
        // and comments. Validate expression contents with the TSX grammar.
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())?;
        let parsed = parse(&mut parser, text.byte_slice(expression.byte_range()), None)?;
        return Ok(jsx::allows_self_close(
            &parsed,
            slash_byte - expression.start_byte(),
        ));
    }
    Ok(true)
}
