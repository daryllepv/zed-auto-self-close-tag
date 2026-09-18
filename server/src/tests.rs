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

#[test]
fn empty_pairs_collapse_without_changing_surrounding_text() {
    for (language, source, expected) in [
        (Language::Tsx, "<p/|></p>", "<p />"),
        (Language::Jsx, "<UI.Button /|></UI.Button>", "<UI.Button />"),
        (Language::Tsx, "<p><p/|></p></p>", "<p><p /></p>"),
        (
            Language::Tsx,
            "<C title=\"😀\"\r\n /|></C><Other />",
            "<C title=\"😀\"\r\n /><Other />",
        ),
        (Language::Tsx, "<ns:Tag/|></ns:Tag>", "<ns:Tag />"),
        (Language::Astro, "<p/|></p>", "<p />"),
        (
            Language::Astro,
            "{show && <Card/|></Card>}",
            "{show && <Card />}",
        ),
        (Language::Svelte, "<Card/|></Card>", "<Card />"),
        (
            Language::Vue,
            "<template><p/|></p></template>",
            "<template><p /></template>",
        ),
        (
            Language::Html,
            "<svg><path/|></path></svg>",
            "<svg><path /></svg>",
        ),
        (
            Language::Xml,
            "<root><item/|></item></root>",
            "<root><item /></root>",
        ),
        (Language::Xml, "<ns:item/|></ns:item\r\n>", "<ns:item />"),
    ] {
        let (text, position) = fixture(source);
        let mut document = Document::new(language, &text).unwrap();
        let fresh = document.complete(position).unwrap();
        let slash = char_offset(&document.text, position).unwrap() - 1;
        let start = position_at(&document.text, slash).unwrap();
        let mut original = document.text.clone();
        original.remove(slash..slash + 1);
        let mut document = Document::new(language, &original.to_string()).unwrap();
        document
            .change(&[TextDocumentContentChangeEvent {
                range: Some(Range::new(start, start)),
                range_length: None,
                text: "/".into(),
            }])
            .unwrap();
        let edit = document
            .complete(position)
            .unwrap()
            .unwrap_or_else(|| panic!("{language:?}: {source}"));
        assert_eq!(Some(edit.clone()), fresh, "{language:?}: {source}");
        assert_eq!(document.text.to_string(), text);
        assert_eq!(document.complete(position).unwrap(), Some(edit.clone()));
        document
            .change(&[TextDocumentContentChangeEvent {
                range: Some(edit.range),
                range_length: None,
                text: edit.new_text,
            }])
            .unwrap();
        assert_eq!(
            document.text.to_string(),
            expected,
            "{language:?}: {source}"
        );
    }
}

#[test]
fn pair_conversion_preserves_content_and_rejects_invalid_contexts() {
    for (language, source) in [
        (Language::Tsx, "<p/|>text</p>"),
        (Language::Tsx, "<p/|> </p>"),
        (Language::Tsx, "<p/|>\n</p>"),
        (Language::Tsx, "<p/|><C /></p>"),
        (Language::Tsx, "<p/|>{value}</p>"),
        (Language::Tsx, "<p/|></other>"),
        (Language::Tsx, "<P/|></p>"),
        (Language::Tsx, "<p/|>"),
        (Language::Tsx, "const s = '<p/|></p>';"),
        (Language::Tsx, "// <p/|></p>"),
        (Language::Tsx, "<C value=\"<p/|></p>\" />"),
        (Language::Html, "<p/|></p>"),
        (Language::Svelte, "<p/|></p>"),
        (Language::Astro, "{ '<p/|></p>' }"),
        (Language::Vue, "<!-- <p/|></p> -->"),
        (Language::Xml, "<item/|></other>"),
        (Language::Xml, "<![CDATA[<item/|></item>]]>"),
        (Language::Xml, "<item/|></item"),
        (Language::Vue, "<script>const s = '<p/|></p>';</script>"),
        (Language::Svelte, "<textarea><Card/|></Card></textarea>"),
        (Language::Tsx, "<p/|><!-- comment --></p>"),
    ] {
        let (text, position) = fixture(source);
        let mut document = Document::new(language, &text).unwrap();
        assert_eq!(
            document.complete(position).unwrap(),
            None,
            "{language:?}: {source}"
        );
        assert_eq!(document.text.to_string(), text);
    }
}
