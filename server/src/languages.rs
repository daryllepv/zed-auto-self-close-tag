//! Supported languages and their grammar-specific completion rules.

mod astro;
mod html;
mod jsx;
mod markup;
mod svelte;
mod vue;
mod xml;

#[cfg(test)]
mod tests;

use ropey::Rope;
use tree_sitter::Tree;

use crate::Error;

/// A language supported by the completion engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    /// TypeScript with JSX syntax.
    Tsx,
    /// JavaScript with JSX syntax.
    Jsx,
    /// Astro component templates.
    Astro,
    /// Svelte component templates.
    Svelte,
    /// Vue single-file components.
    Vue,
    /// HTML, including SVG and MathML foreign content.
    Html,
    /// XML documents.
    Xml,
}

impl Language {
    /// Resolves an LSP language identifier. Unsupported languages are ignored.
    pub fn from_language_id(id: &str) -> Option<Self> {
        match id {
            "typescriptreact" | "tsx" => Some(Self::Tsx),
            "javascript" | "javascriptreact" | "jsx" => Some(Self::Jsx),
            "astro" => Some(Self::Astro),
            "svelte" => Some(Self::Svelte),
            "vue" => Some(Self::Vue),
            "html" => Some(Self::Html),
            "xml" => Some(Self::Xml),
            _ => None,
        }
    }

    pub(crate) fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Jsx => tree_sitter_javascript::LANGUAGE.into(),
            Self::Astro => tree_sitter_astro_next::LANGUAGE.into(),
            Self::Svelte => tree_sitter_svelte_ng::LANGUAGE.into(),
            Self::Vue => tree_sitter_vue_next::LANGUAGE.into(),
            Self::Html => tree_sitter_html::LANGUAGE.into(),
            Self::Xml => tree_sitter_xml::LANGUAGE_XML.into(),
        }
    }

    // The tree includes the proposed `>`. Each language decides whether the
    // delimiter belongs to a construct that may legally self-close.
    pub(crate) fn allows_self_close(
        self,
        tree: &Tree,
        text: &Rope,
        slash_byte: usize,
    ) -> Result<bool, Error> {
        Ok(match self {
            Self::Tsx | Self::Jsx => jsx::allows_self_close(tree, slash_byte),
            Self::Astro => return astro::allows_self_close(tree, text, slash_byte),
            Self::Svelte => svelte::allows_self_close(tree, text, slash_byte),
            Self::Vue => vue::allows_self_close(tree, text, slash_byte),
            Self::Html => html::allows_self_close(tree, text, slash_byte),
            Self::Xml => xml::allows_self_close(tree, text, slash_byte),
        })
    }
}
