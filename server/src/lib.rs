//! Incremental editing and self-closing tag completion, independent of LSP transport.

mod languages;

pub use languages::Language;

use lsp_types::{Position, Range, TextDocumentContentChangeEvent, TextEdit};
use ropey::{Rope, RopeSlice};
use tree_sitter::{InputEdit, Parser, Point, Tree};

/// Failures that prevent a document from being parsed or updated safely.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The bundled grammar is incompatible with the parser.
    #[error(transparent)]
    Grammar(#[from] tree_sitter::LanguageError),
    /// A change refers to a nonexistent position or splits a UTF-16 surrogate pair.
    #[error("invalid UTF-16 edit range")]
    InvalidRange,
    /// Tree-sitter could not produce a syntax tree.
    #[error("syntax parsing did not complete")]
    Parse,
}

/// An open document with a language-specific parser and incremental text buffer.
pub struct Document {
    language: Language,
    text: Rope,
    tree: Tree,
    parser: Parser,
}

impl Document {
    /// Parses the initial contents. Subsequent edits reuse unchanged syntax.
    ///
    /// # Errors
    /// Returns an error if the grammar cannot be loaded or parsing fails.
    pub fn new(language: Language, text: &str) -> Result<Self, Error> {
        let text = Rope::from_str(text);
        let mut parser = Parser::new();
        parser.set_language(&language.grammar())?;
        let tree = parse(&mut parser, text.slice(..), None)?;
        Ok(Self {
            language,
            text,
            tree,
            parser,
        })
    }

    /// Applies an ordered LSP change batch atomically. Ranges use UTF-16 columns.
    /// A change without a range replaces the entire document.
    /// Columns beyond a line's content clamp to its end, as required by LSP.
    ///
    /// # Errors
    /// Rejects invalid ranges without changing the document.
    pub fn change(&mut self, changes: &[TextDocumentContentChangeEvent]) -> Result<(), Error> {
        // Rope and Tree clones share storage; staging edits makes a batch atomic.
        let mut text = self.text.clone();
        let mut tree = self.tree.clone();
        for change in changes {
            let (start, end) = match change.range {
                Some(range) => (
                    char_offset(&text, range.start).ok_or(Error::InvalidRange)?,
                    char_offset(&text, range.end).ok_or(Error::InvalidRange)?,
                ),
                None => (0, text.len_chars()),
            };
            if start > end {
                return Err(Error::InvalidRange);
            }
            replace(&mut text, &mut tree, start..end, &change.text);
        }
        self.text = text;
        self.tree = tree;
        Ok(())
    }

    /// Completes a slash immediately before a zero-based UTF-16 position.
    /// Collapses empty tag pairs when `>` already exists; otherwise completes the delimiter.
    /// Returns no edit outside supported opening tags or for invalid positions.
    /// The document is unchanged until the client sends the resulting change.
    ///
    /// # Errors
    /// Returns an error if the candidate syntax cannot be parsed.
    pub fn complete(&mut self, position: Position) -> Result<Option<TextEdit>, Error> {
        let Some(offset) = char_offset(&self.text, position) else {
            return Ok(None);
        };
        if offset == 0 || self.text.get_char(offset - 1) != Some('/') {
            return Ok(None);
        }
        if self.text.get_char(offset) == Some('>') {
            return self.collapse_pair(offset);
        }
        let line_start = self.text.line_to_char(position.line as usize);
        let column = self.text.char_to_utf16_cu(offset) - self.text.char_to_utf16_cu(line_start);
        let Ok(column) = u32::try_from(column) else {
            return Ok(None);
        };
        let position = Position::new(position.line, column);
        let Some(column) = column.checked_sub(1) else {
            return Ok(None);
        };
        let byte = self.text.char_to_byte(offset);
        let at = point(&self.text, offset);
        let mut text = self.text.clone();
        let mut tree = self.tree.clone();
        text.insert(offset, ">");
        tree.edit(&InputEdit {
            start_byte: byte,
            old_end_byte: byte,
            new_end_byte: byte + 1,
            start_position: at,
            old_end_position: at,
            new_end_position: Point::new(at.row, at.column + 1),
        });
        // Incomplete tags may be ERROR nodes. Validate the proposed delimiter with
        // the grammar, reusing the real document's tree and copy-on-write text.
        let mut tree = parse(&mut self.parser, text.slice(..), Some(&tree))?;
        let valid = self.language.allows_self_close(&tree, &text, byte - 1)?;
        // Remove the speculative character from the cached tree's coordinates.
        // Tree-sitter will reparse that edited region on the next completion.
        tree.edit(&InputEdit {
            start_byte: byte,
            old_end_byte: byte + 1,
            new_end_byte: byte,
            start_position: at,
            old_end_position: Point::new(at.row, at.column + 1),
            new_end_position: at,
        });
        self.tree = tree;
        if !valid {
            return Ok(None);
        }
        // Zed pins the cursor before an on-type insertion. Replacing the typed
        // slash lets it follow the delimiter; LSP has no cursor-placement field.
        Ok(Some(TextEdit {
            range: Range::new(Position::new(position.line, column), position),
            new_text: "/>".into(),
        }))
    }

    fn collapse_pair(&mut self, offset: usize) -> Result<Option<TextEdit>, Error> {
        if self.text.get_char(offset + 1) != Some('<')
            || self.text.get_char(offset + 2) != Some('/')
        {
            return Ok(None);
        }
        let slash = offset - 1;
        let mut text = self.text.clone();
        let mut tree = self.tree.clone();
        // Recover the paired syntax before the slash changed the opening tag.
        replace(&mut text, &mut tree, slash..offset, "");
        let mut tree = parse(&mut self.parser, text.slice(..), Some(&tree))?;
        let Some(end_byte) = self
            .language
            .empty_pair_end(&tree, &text, text.char_to_byte(slash))
        else {
            return Ok(None);
        };
        let end = text.byte_to_char(end_byte);
        let new_text = if slash > 0 && text.char(slash - 1).is_whitespace() {
            "/>"
        } else {
            " />"
        };
        replace(&mut text, &mut tree, slash..end, new_text);
        let tree = parse(&mut self.parser, text.slice(..), Some(&tree))?;
        let slash_byte = text.char_to_byte(slash) + new_text.len() - 2;
        if !self.language.allows_self_close(&tree, &text, slash_byte)? {
            return Ok(None);
        }
        let Some(start) = position_at(&self.text, slash) else {
            return Ok(None);
        };
        let Some(end) = position_at(&self.text, end + 1) else {
            return Ok(None);
        };
        Ok(Some(TextEdit {
            range: Range::new(start, end),
            new_text: new_text.into(),
        }))
    }
}

fn parse(parser: &mut Parser, text: RopeSlice<'_>, old_tree: Option<&Tree>) -> Result<Tree, Error> {
    parser
        .parse_with_options(
            &mut |byte, _| -> &[u8] {
                if byte >= text.len_bytes() {
                    return &[];
                }
                let (chunk, start, _, _) = text.chunk_at_byte(byte);
                &chunk.as_bytes()[byte - start..]
            },
            old_tree,
            None,
        )
        .ok_or(Error::Parse)
}

fn char_offset(text: &Rope, position: Position) -> Option<usize> {
    let line = text.get_line(position.line as usize)?;
    let mut length = line.len_chars();
    if length > 0 && line.get_char(length - 1) == Some('\n') {
        length -= 1;
        if length > 0 && line.get_char(length - 1) == Some('\r') {
            length -= 1;
        }
    }
    let units = (position.character as usize).min(line.char_to_utf16_cu(length));
    let index = line.try_utf16_cu_to_char(units).ok()?;
    if line.char_to_utf16_cu(index) != units {
        return None;
    }
    Some(text.line_to_char(position.line as usize) + index)
}

fn point(text: &Rope, char_index: usize) -> Point {
    let row = text.char_to_line(char_index);
    Point::new(row, text.char_to_byte(char_index) - text.line_to_byte(row))
}

fn point_after(mut start: Point, text: &str) -> Point {
    for byte in text.bytes() {
        if byte == b'\n' {
            start.row += 1;
            start.column = 0;
        } else {
            start.column += 1;
        }
    }
    start
}

fn replace(text: &mut Rope, tree: &mut Tree, range: std::ops::Range<usize>, inserted: &str) {
    let start_byte = text.char_to_byte(range.start);
    let start_position = point(text, range.start);
    tree.edit(&InputEdit {
        start_byte,
        old_end_byte: text.char_to_byte(range.end),
        new_end_byte: start_byte + inserted.len(),
        start_position,
        old_end_position: point(text, range.end),
        new_end_position: point_after(start_position, inserted),
    });
    text.remove(range.clone());
    text.insert(range.start, inserted);
}

fn position_at(text: &Rope, offset: usize) -> Option<Position> {
    let line = text.char_to_line(offset);
    let column = text.char_to_utf16_cu(offset) - text.char_to_utf16_cu(text.line_to_char(line));
    Some(Position::new(
        u32::try_from(line).ok()?,
        u32::try_from(column).ok()?,
    ))
}

#[cfg(test)]
mod tests;
