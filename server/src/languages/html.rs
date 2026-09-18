use ropey::Rope;
use tree_sitter::{Node, Tree};

use super::markup;

#[derive(Clone, Copy, PartialEq)]
enum Namespace {
    Html,
    Svg,
    MathMl,
}

pub(super) fn allows_self_close(tree: &Tree, text: &Rope, slash_byte: usize) -> bool {
    markup::self_closing_tag(tree, slash_byte).is_some_and(|tag| tag_may_self_close(tag, text))
}

pub(super) fn tag_may_self_close(tag: Node<'_>, text: &Rope) -> bool {
    let mut namespace = Namespace::Html;
    let mut parent = None;
    for current in markup::ancestor_start_tags(tag).into_iter().chain([tag]) {
        let name = markup::tag_name(current, text).to_ascii_lowercase();
        if let Some(parent) = parent {
            let parent_name = markup::tag_name(parent, text).to_ascii_lowercase();
            match namespace {
                Namespace::Html if is_raw_text(&parent_name) => return false,
                Namespace::Svg
                    if matches!(parent_name.as_str(), "foreignobject" | "desc" | "title") =>
                {
                    namespace = Namespace::Html;
                }
                Namespace::MathMl
                    if matches!(parent_name.as_str(), "mi" | "mo" | "mn" | "ms" | "mtext")
                        && !matches!(name.as_str(), "mglyph" | "malignmark") =>
                {
                    namespace = Namespace::Html;
                }
                Namespace::MathMl if parent_name == "annotation-xml" => {
                    if name == "svg" {
                        namespace = Namespace::Svg;
                    } else if markup::attribute(parent, text, "encoding").is_some_and(|encoding| {
                        encoding.eq_ignore_ascii_case("text/html")
                            || encoding.eq_ignore_ascii_case("application/xhtml+xml")
                    }) {
                        namespace = Namespace::Html;
                    }
                }
                _ => {}
            }
        }
        // HTML start tags can exit foreign content even without an explicit
        // closing SVG/MathML tag. Tree-sitter does not model HTML namespaces.
        if namespace != Namespace::Html && breaks_out_of_foreign_content(&name, current, text) {
            namespace = Namespace::Html;
        }
        if namespace == Namespace::Html {
            namespace = match name.as_str() {
                "svg" => Namespace::Svg,
                "math" => Namespace::MathMl,
                _ => Namespace::Html,
            };
        }
        if current == tag {
            return namespace != Namespace::Html || is_void(&name);
        }
        parent = Some(current);
    }
    false
}

fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_raw_text(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "textarea"
            | "title"
            | "xmp"
            | "iframe"
            | "noembed"
            | "noframes"
            | "noscript"
            | "plaintext"
    )
}

fn breaks_out_of_foreign_content(name: &str, tag: Node<'_>, text: &Rope) -> bool {
    matches!(
        name,
        "b" | "big"
            | "blockquote"
            | "body"
            | "br"
            | "center"
            | "code"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "em"
            | "embed"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "hr"
            | "i"
            | "img"
            | "li"
            | "listing"
            | "menu"
            | "meta"
            | "nobr"
            | "ol"
            | "p"
            | "pre"
            | "ruby"
            | "s"
            | "small"
            | "span"
            | "strong"
            | "strike"
            | "sub"
            | "sup"
            | "table"
            | "tt"
            | "u"
            | "ul"
            | "var"
    ) || name == "font"
        && ["color", "face", "size"]
            .iter()
            .any(|name| markup::attribute(tag, text, name).is_some())
}
