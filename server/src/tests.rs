#![expect(
    clippy::unwrap_used,
    reason = "test fixtures and assertions may fail loudly"
)]

use super::*;

fn fixture(marked: &str) -> (String, Position) {
    let (before, after) = marked.split_once('|').unwrap();
    let position = Position::new(
        before.bytes().filter(|&byte| byte == b'\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    );
    (format!("{before}{after}"), position)
}

#[test]
fn tsx_completion_edits_only_self_closing_tag_delimiters() {
    for (source, expected) in [
        ("<Component /|", Some("<Component />")),
        (
            "const view = <Component/|;",
            Some("const view = <Component/>;"),
        ),
        ("<UI.Button {...props} /|", Some("<UI.Button {...props} />")),
        (
            "<C title=\"😀\" value={a / 2}\r\n /|",
            Some("<C title=\"😀\" value={a / 2}\r\n />"),
        ),
        ("<C title=\"😀\" /|", Some("<C title=\"😀\" />")),
        ("<div><C /|</div>", Some("<div><C /></div>")),
        ("<C /|>", None),
        ("</|", None),
        ("<C path=\"/|\"", None),
        ("<C value={a /|", None),
        ("const ratio = a /|", None),
        ("// <C /|", None),
        ("/* <C /| */", None),
        ("const text = '<C /|';", None),
        ("const text = `<C /|`;", None),
        ("const regex = /<C /|;", None),
        ("<C title=/|", None),
    ] {
        let (text, position) = fixture(source);
        let mut document = Document::new(Language::Tsx, &text).unwrap();
        let edit = document.complete(position).unwrap();
        assert_eq!(
            document.text.to_string(),
            text,
            "completion mutated {source}"
        );
        let actual = edit.map(|edit| {
            assert_eq!(edit.range.end, position, "{source}");
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            document.text.to_string()
        });
        assert_eq!(actual.as_deref(), expected, "{source}");
    }
}

#[test]
fn incremental_completion_matches_fresh_parse_through_typing_and_replacement() {
    let mut document = Document::new(Language::Tsx, "const view = <C title=\"😀\" ").unwrap();
    let mut current = "const view = <C title=\"😀\" ".to_owned();
    for inserted in ["/", ">", "\n", "<Child ", "/"] {
        let (_, position) = fixture(&format!("{current}|"));
        document
            .change(&[TextDocumentContentChangeEvent {
                range: Some(Range::new(position, position)),
                range_length: None,
                text: inserted.into(),
            }])
            .unwrap();
        current.push_str(inserted);
        let (_, position) = fixture(&format!("{current}|"));
        assert_eq!(
            document.complete(position).unwrap(),
            Document::new(Language::Tsx, &current)
                .unwrap()
                .complete(position)
                .unwrap(),
            "after {inserted:?}"
        );
    }
    let (_, end) = fixture(&format!("{current}|"));
    let start = Position::new(end.line, end.character - 1);
    document
        .change(&[
            TextDocumentContentChangeEvent {
                range: Some(Range::new(start, end)),
                range_length: None,
                text: String::new(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(start, start)),
                range_length: None,
                text: "/".into(),
            },
        ])
        .unwrap();
    assert_eq!(
        document.complete(end).unwrap(),
        Document::new(Language::Tsx, &current)
            .unwrap()
            .complete(end)
            .unwrap()
    );
    document
        .change(&[TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "// <C /".into(),
        }])
        .unwrap();
    assert_eq!(document.complete(Position::new(0, 7)).unwrap(), None);
}

#[test]
fn invalid_utf16_or_reversed_ranges_reject_the_entire_batch() {
    let text = "//😀\r\n<C /";
    for range in [
        Range::new(Position::new(0, 3), Position::new(0, 4)),
        Range::new(Position::new(4, 0), Position::new(4, 0)),
        Range::new(Position::new(1, 2), Position::new(1, 1)),
    ] {
        let mut document = Document::new(Language::Tsx, text).unwrap();
        let result = document.change(&[
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 0), Position::new(1, 0))),
                range_length: None,
                text: " ".into(),
            },
            TextDocumentContentChangeEvent {
                range: Some(range),
                range_length: None,
                text: "x".into(),
            },
        ]);
        assert!(matches!(result, Err(Error::InvalidRange)), "{range:?}");
        assert_eq!(document.text.to_string(), text);
        assert_eq!(
            document.complete(Position::new(1, 4)).unwrap(),
            Some(TextEdit {
                range: Range::new(Position::new(1, 3), Position::new(1, 4)),
                new_text: "/>".into(),
            })
        );
    }
    let rope = Rope::from_str(text);
    assert_eq!(char_offset(&rope, Position::new(0, 4)), Some(3));
    assert_eq!(char_offset(&rope, Position::new(1, 4)), Some(9));
}

#[test]
fn oversized_columns_clamp_before_line_endings_and_return_normalized_edits() {
    for ending in ["", "\n", "\r\n"] {
        let text = format!("<C title=\"😀\" {ending}");
        let mut document = Document::new(Language::Tsx, &text).unwrap();
        document
            .change(&[TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 999), Position::new(0, 999))),
                range_length: None,
                text: "/".into(),
            }])
            .unwrap();
        assert_eq!(
            document.text.to_string(),
            format!("<C title=\"😀\" /{ending}")
        );
        assert_eq!(
            document.complete(Position::new(0, 999)).unwrap(),
            Some(TextEdit {
                range: Range::new(Position::new(0, 14), Position::new(0, 15)),
                new_text: "/>".into(),
            }),
            "line ending {ending:?}"
        );
    }
    let mut document = Document::new(Language::Tsx, "😀\r\n<C /").unwrap();
    document
        .change(&[TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 0), Position::new(0, 999))),
            range_length: None,
            text: String::new(),
        }])
        .unwrap();
    assert_eq!(document.text.to_string(), "\r\n<C /");
}

#[test]
fn completion_can_be_applied_undone_and_reapplied() {
    let mut document = Document::new(Language::Tsx, "<Component /").unwrap();
    let position = Position::new(0, 12);
    let edit = document.complete(position).unwrap().unwrap();
    document
        .change(&[TextDocumentContentChangeEvent {
            range: Some(edit.range),
            range_length: None,
            text: edit.new_text.clone(),
        }])
        .unwrap();
    assert_eq!(document.text.to_string(), "<Component />");
    assert_eq!(document.complete(position).unwrap(), None);

    document
        .change(&[TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(0, 11), Position::new(0, 13))),
            range_length: None,
            text: "/".into(),
        }])
        .unwrap();
    assert_eq!(document.text.to_string(), "<Component /");
    assert_eq!(document.complete(position).unwrap(), Some(edit.clone()));
    document
        .change(&[TextDocumentContentChangeEvent {
            range: Some(edit.range),
            range_length: None,
            text: edit.new_text,
        }])
        .unwrap();
    assert_eq!(document.text.to_string(), "<Component />");
}

#[test]
fn earlier_multiline_edits_preserve_completion_at_shifted_positions() {
    let mut document = Document::new(Language::Tsx, "<C /").unwrap();
    assert!(document.complete(Position::new(0, 4)).unwrap().is_some());
    for (range, inserted, expected) in [
        (
            Range::new(Position::new(0, 0), Position::new(0, 0)),
            "const face = \"😀\";\r\n",
            "const face = \"😀\";\r\n<C /|",
        ),
        (
            Range::new(Position::new(0, 0), Position::new(1, 0)),
            "/* header\nsecond line */\n",
            "/* header\nsecond line */\n<C /|",
        ),
        (
            Range::new(Position::new(0, 0), Position::new(2, 0)),
            "",
            "<C /|",
        ),
    ] {
        document
            .change(&[TextDocumentContentChangeEvent {
                range: Some(range),
                range_length: None,
                text: inserted.into(),
            }])
            .unwrap();
        let (text, position) = fixture(expected);
        assert_eq!(document.text.to_string(), text);
        let fresh = Document::new(Language::Tsx, &text)
            .unwrap()
            .complete(position)
            .unwrap();
        assert!(fresh.is_some(), "{expected}");
        assert_eq!(document.complete(position).unwrap(), fresh, "{expected}");
    }
}
