#![expect(
    clippy::unwrap_used,
    reason = "test fixtures and assertions may fail loudly"
)]

use lsp_types::{Position, Range, TextDocumentContentChangeEvent, TextEdit};

use crate::{Document, Language};

fn fixture(language: Language, marked: &str) -> (Document, Position) {
    let (before, after) = marked.split_once('|').unwrap();
    let position = Position::new(
        before.bytes().filter(|&byte| byte == b'\n').count() as u32,
        before.rsplit('\n').next().unwrap().encode_utf16().count() as u32,
    );
    (
        Document::new(language, &format!("{before}{after}")).unwrap(),
        position,
    )
}

#[test]
fn jsx_completes_elements_but_not_javascript_expressions_or_literals() {
    for (source, expected) in [
        ("const view = <Button /|;", true),
        ("const view = <UI.Button {...props} /|;", true),
        ("const view = <><custom-element data-id=\"😀\" /|</>;", true),
        ("const view = <svg><path d={value} /|</svg>;", true),
        ("const view = <C value={left /| right} />;", false),
        ("const view = <C value=\"/|\" />;", false),
        ("const value = '<Button /|';", false),
        ("const value = `<Button /|`;", false),
        ("const value = /<Button /|;", false),
        ("// <Button /|", false),
        ("/* <Button /| */", false),
        ("const value = left /| right;", false),
    ] {
        let (mut document, position) = fixture(Language::Jsx, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(edit.is_some(), expected, "{source}");
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn astro_distinguishes_template_tags_from_frontmatter_and_javascript() {
    for (source, expected) in [
        ("<Card /|", true),
        (
            "---\nimport Card from './Card.astro';\n---\n<Card client:load /|",
            true,
        ),
        ("<div class:list={['a', {active}]} /|", true),
        ("<C {...props} title=`hello ${name}` /|", true),
        ("<C title=\"😀\"\r\n /|", true),
        ("{items.map(item => <Card title={item.title} /|)}", true),
        ("{show && <Card /|}", true),
        ("---\nconst text = '<Card /|';\n---", false),
        ("---\n// <Card /|\n---", false),
        ("<script>const text = '<Card /|';</script>", false),
        ("<style>.x { content: '<Card /|'; }</style>", false),
        ("<!-- <Card /| -->", false),
        ("{ '<Card /|' }", false),
        ("{ `<Card /|` }", false),
        ("{ /* <Card /| */ }", false),
        ("{ value /| other }", false),
        ("<C value={left /| right}", false),
        ("<C value=\"<Card /|\"", false),
        ("<C value=`<Card /|`", false),
        ("<C value=/|", false),
        ("<textarea><Card /|</textarea>", false),
        ("<!-- <Card /|", false),
        ("{ 'unfinished <Card /|", false),
    ] {
        let (mut document, position) = fixture(Language::Astro, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(edit.is_some(), expected, "{source}");
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn svelte_completes_components_special_elements_and_void_or_foreign_tags() {
    for (source, expected) in [
        ("<Card /|", true),
        ("<item.component {...item.props} /|", true),
        ("<input bind:value={value} /|", true),
        ("<svelte:component this={Card} /|", true),
        ("<svelte:window onresize={resize} /|", true),
        ("<slot name=\"header\" /|", true),
        ("{#if show}<Card /|{/if}", true),
        ("{#each items as item}<Card item={item} /|{/each}", true),
        ("{#snippet row(item)}<Card {item} /|{/snippet}", true),
        ("<svg><path /|</svg>", true),
        ("<div /|", false),
        ("<custom-element /|", false),
        (
            "<script lang=\"ts\">const text = '<Card /|';</script>",
            false,
        ),
        ("<style>.x { content: '<Card /|'; }</style>", false),
        ("<!-- <Card /| -->", false),
        ("{ '<Card /|' }", false),
        ("{ `hello ${'<Card /|'}` }", false),
        ("{ /* <Card /| */ value }", false),
        ("{#if value /| other}<Card />{/if}", false),
        ("<Card value={left /| right}", false),
        ("<Card value=\"<Card /|\"", false),
        ("<Card value=/|", false),
        ("<textarea><Card /|</textarea>", false),
        ("<!-- <Card /|", false),
        ("{ 'unfinished <Card /|", false),
    ] {
        let (mut document, position) = fixture(Language::Svelte, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(edit.is_some(), expected, "{source}");
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn vue_completes_html_templates_but_not_scripts_styles_or_other_template_languages() {
    for (source, expected) in [
        ("<template><Card /|</template>", true),
        (
            "<template><my-card v-if=\"show\" :value=\"a / b\" @click=\"run\" /|</template>",
            true,
        ),
        ("<template><component :is=\"active\" /|</template>", true),
        ("<template><div /|</template>", true),
        ("<template lang=\"html\"><Card /|</template>", true),
        (
            "<template><template #default><Card /|</template></template>",
            true,
        ),
        ("<template><Card :[key]=\"value\" /|</template>", true),
        (
            "<script setup lang=\"ts\">const text = '<Card /|';</script>",
            false,
        ),
        ("<style scoped>.x { content: '<Card /|'; }</style>", false),
        ("<template><!-- <Card /| --></template>", false),
        ("<template>{{ '<Card /|' }}</template>", false),
        ("<template>{{ left /| right }}</template>", false),
        (
            "<template><Card :value=\"left /| right\" /></template>",
            false,
        ),
        ("<template><Card value=\"<Card /|\" /></template>", false),
        ("<template lang=\"pug\">p <Card /|</template>", false),
        ("<i18n>{\"message\": \"<Card /|\"}</i18n>", false),
        ("<template><Card value=/|</template>", false),
        ("<template><textarea><Card /|</textarea></template>", false),
        ("<template><!-- <Card /|", false),
        ("<template>{{ 'unfinished <Card /|", false),
    ] {
        let (mut document, position) = fixture(Language::Vue, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(
            edit.is_some(),
            expected,
            "{source}: {}",
            document.tree.root_node().to_sexp()
        );
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn html_respects_void_elements_foreign_namespaces_and_integration_points() {
    for (source, expected) in [
        ("<img /|", true),
        ("<INPUT disabled /|", true),
        ("<img src=/images/photo /|", true),
        ("<svg /|", true),
        ("<svg><path /|</svg>", true),
        ("<math><mspace /|</math>", true),
        ("<svg><foreignObject /|</svg>", true),
        ("<svg><foreignObject><input /|</foreignObject></svg>", true),
        (
            "<svg><foreignObject><svg><path /|</svg></foreignObject></svg>",
            true,
        ),
        ("<math><mtext><mglyph /|</mtext></math>", true),
        (
            "<math><annotation-xml encoding=\"application/xml\"><thing /|</annotation-xml></math>",
            true,
        ),
        ("<div /|", false),
        ("<custom-element /|", false),
        ("<path /|", false),
        ("<img src=/images/|", false),
        ("<img src=\"/images/|\"", false),
        ("<img src=/|", false),
        ("<!-- <img /| -->", false),
        ("<script>const text = '<img /|';</script>", false),
        ("<style>.x { content: '<img /|'; }</style>", false),
        ("<textarea><img /|</textarea>", false),
        ("<title><img /|</title>", false),
        ("<svg><foreignObject><div /|</foreignObject></svg>", false),
        ("<svg><desc><div /|</desc></svg>", false),
        ("<svg><div /|</svg>", false),
        ("<math><mtext><div /|</mtext></math>", false),
        (
            "<math><annotation-xml encoding=\"text/html\"><div /|</annotation-xml></math>",
            false,
        ),
        (
            "<math><annotation-xml encoding=\"TEXT/HTML\"><path /|</annotation-xml></math>",
            false,
        ),
    ] {
        let (mut document, position) = fixture(Language::Html, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(edit.is_some(), expected, "{source}");
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn xml_completes_named_elements_but_not_cdata_declarations_or_attributes() {
    for (source, expected) in [
        ("<root /|", true),
        ("<?xml version=\"1.0\"?><root><item /|</root>", true),
        (
            "<root xmlns:x=\"urn:example\"><x:item name=\"😀\" /|</root>",
            true,
        ),
        ("<root><1item /|</root>", false),
        ("<root><!-- <item /| --></root>", false),
        ("<root><![CDATA[<item /|]]></root>", false),
        ("<?processing <item /| ?> <root />", false),
        (
            "<!DOCTYPE root [<!ENTITY sample \"<item /|\">]><root />",
            false,
        ),
        ("<root path=\"/|\"", false),
        ("<root boolean /|", false),
        ("<root value=unquoted /|", false),
        ("<root><!-- <item /|", false),
        ("<root><![CDATA[<item /|", false),
    ] {
        let (mut document, position) = fixture(Language::Xml, source);
        let edit = document.complete(position).unwrap();
        assert_eq!(edit.is_some(), expected, "{source}");
        if let Some(edit) = edit {
            document
                .change(&[TextDocumentContentChangeEvent {
                    range: Some(edit.range),
                    range_length: None,
                    text: edit.new_text,
                }])
                .unwrap();
            assert_eq!(
                document.text.to_string(),
                source.replace('|', ">"),
                "{source}"
            );
        }
    }
}

#[test]
fn all_languages_preserve_unicode_positions_and_do_not_duplicate_delimiters() {
    for language in [
        Language::Tsx,
        Language::Jsx,
        Language::Astro,
        Language::Svelte,
        Language::Vue,
        Language::Html,
        Language::Xml,
    ] {
        let (mut document, position) = fixture(language, "<input title=\"😀\"\r\n /|");
        assert_eq!(
            document.complete(position).unwrap(),
            Some(TextEdit {
                range: Range::new(Position::new(1, 1), Position::new(1, 2)),
                new_text: "/>".into(),
            }),
            "{language:?}"
        );
        for source in ["<input /|>", "</|", "< /|", "<input value=\"/|\""] {
            let (mut document, position) = fixture(language, source);
            assert_eq!(
                document.complete(position).unwrap(),
                None,
                "{language:?}: {source}"
            );
        }
    }
}
