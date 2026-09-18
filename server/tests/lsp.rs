//! End-to-end tests of the native server's LSP stdio interface.
#![expect(
    clippy::unwrap_used,
    reason = "test setup and protocol assertions should fail loudly"
)]

use std::{
    io::{BufReader, Read},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use lsp_server::{Message, Notification, Request, Response};
use serde_json::{Value, json};

const URI: &str = "file:///smoke.tsx";

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: Receiver<Message>,
    stderr: Option<JoinHandle<String>>,
    next_id: i32,
}

impl Server {
    fn start() -> (Self, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_auto-self-close-tag-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stderr = child.stderr.take().unwrap();
        let (tx, responses) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(Some(message)) = Message::read(&mut stdout) {
                if tx.send(message).is_err() {
                    break;
                }
            }
        });
        let stderr = thread::spawn(move || {
            let mut output = String::new();
            stderr.read_to_string(&mut output).unwrap();
            output
        });
        let mut server = Self {
            child,
            stdin,
            responses,
            stderr: Some(stderr),
            next_id: 0,
        };
        let response = server.request("initialize", json!({"processId": null, "capabilities": {}}));
        server.notify("initialized", json!({}));
        (server, response.result.unwrap())
    }

    fn send(&mut self, message: Message) {
        message.write(self.stdin.as_mut().unwrap()).unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Response {
        self.next_id += 1;
        let id = lsp_server::RequestId::from(self.next_id);
        self.send(Message::Request(Request {
            id: id.clone(),
            method: method.into(),
            params,
        }));
        let Message::Response(response) = self
            .responses
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        else {
            panic!("expected an LSP response");
        };
        assert_eq!(response.id, id);
        response
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(Message::Notification(Notification {
            method: method.into(),
            params,
        }));
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": "typescriptreact", "version": 1, "text": text
            }}),
        );
    }

    fn change(&mut self, uri: &str, version: i32, changes: Value) {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": version}, "contentChanges": changes
            }),
        );
    }

    fn format(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.request(
            "textDocument/onTypeFormatting",
            json!({
                "textDocument": {"uri": uri}, "position": {"line": line, "character": character},
                "ch": "/", "options": {"tabSize": 2, "insertSpaces": true}
            }),
        )
        .result
        .unwrap()
    }

    fn finish(mut self) -> String {
        let response = self.request("shutdown", Value::Null);
        assert!(response.error.is_none());
        // Serde deserializes the LSP null result into Option::None.
        assert!(response.result.is_none());
        self.notify("exit", Value::Null);
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "server exit: {status}");
                break;
            }
            assert!(Instant::now() < deadline, "server did not exit");
            thread::sleep(Duration::from_millis(10));
        }
        self.stderr.take().unwrap().join().unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn stdio_typing_lifecycle_preserves_edits_versions_and_close() {
    let (mut server, initialized) = Server::start();
    assert_eq!(
        initialized["capabilities"],
        json!({
            "positionEncoding": "utf-16",
            "textDocumentSync": {"openClose": true, "change": 2},
            "documentOnTypeFormattingProvider": {"firstTriggerCharacter": "/"}
        })
    );
    server.open(URI, "<Component ");
    server.change(URI, 2, json!([{
        "range": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 11}}, "text": "/"
    }]));
    let edit = server.format(URI, 0, 12);
    assert_eq!(
        edit,
        json!([{
            "range": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 12}}, "newText": "/>"
        }])
    );
    server.change(
        URI,
        3,
        json!([{"range": edit[0]["range"], "text": edit[0]["newText"]}]),
    );
    assert_eq!(server.format(URI, 0, 12), json!([]));
    server.change(URI, 2, json!([{"text": "<Component /"}]));
    assert_eq!(server.format(URI, 0, 12), json!([]));
    server.notify(
        "textDocument/didClose",
        json!({"textDocument": {"uri": URI}}),
    );
    assert_eq!(server.format(URI, 0, 12), json!([]));
    assert!(server.finish().is_empty());
}

#[test]
fn invalid_requests_return_protocol_errors_without_stopping_the_server() {
    let (mut server, _) = Server::start();
    assert_eq!(
        server
            .request("textDocument/onTypeFormatting", json!({}))
            .error
            .unwrap()
            .code,
        -32602
    );
    assert_eq!(
        server
            .request("unsupported/method", json!({}))
            .error
            .unwrap()
            .code,
        -32601
    );
    assert!(server.finish().is_empty());
}

#[test]
fn invalid_change_suspends_only_affected_document_until_full_resync() {
    let (mut server, _) = Server::start();
    let other = "file:///other.tsx";
    server.open(URI, "<Component /");
    server.open(other, "<Component /");
    server.change(URI, 3, json!([{
        "range": {"start": {"line": 99, "character": 0}, "end": {"line": 99, "character": 0}}, "text": "x"
    }]));
    assert_eq!(server.format(URI, 0, 12), json!([]));
    assert_eq!(server.format(other, 0, 12)[0]["newText"], "/>");
    server.change(URI, 2, json!([{"text": "<Component /"}]));
    assert_eq!(server.format(URI, 0, 12), json!([]));
    server.change(URI, 4, json!([{"text": "<Component /"}]));
    assert_eq!(server.format(URI, 0, 12)[0]["newText"], "/>");
    assert!(server.finish().contains("invalid UTF-16 edit range"));
}

#[test]
fn oversized_change_and_format_columns_use_the_actual_line_end() {
    let (mut server, _) = Server::start();
    server.open(URI, "<Component \r\n");
    server.change(URI, 2, json!([{
        "range": {"start": {"line": 0, "character": 999}, "end": {"line": 0, "character": 999}}, "text": "/"
    }]));
    assert_eq!(
        server.format(URI, 0, 999),
        json!([{
            "range": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 12}}, "newText": "/>"
        }])
    );
    assert!(server.finish().is_empty());
}

#[test]
fn every_language_handles_typing_undo_and_full_resynchronization() {
    let (mut server, _) = Server::start();
    for (language_id, marked) in [
        ("typescriptreact", "const view = <Card |;"),
        ("tsx", "const view = <Card |;"),
        ("javascript", "const view = <Card |;"),
        ("javascriptreact", "const view = <Card |;"),
        ("jsx", "const view = <Card |;"),
        (
            "astro",
            "---\nconst ready = true;\n---\n<Card client:load |",
        ),
        ("svelte", "{#if ready}<Card {value} |{/if}"),
        ("vue", "<template><my-card v-if=\"ready\" |</template>"),
        ("html", "<svg><path d=\"M0 0\" |</svg>"),
        ("xml", "<?xml version=\"1.0\"?><root><item |</root>"),
    ] {
        let (before, after) = marked.split_once('|').unwrap();
        let line = before.bytes().filter(|&byte| byte == b'\n').count() as u32;
        let column = before.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
        let uri = format!("file:///test.{language_id}");
        server.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": language_id, "version": 1, "text": format!("{before}{after}")
            }}),
        );
        server.change(&uri, 2, json!([{
            "range": {"start": {"line": line, "character": column}, "end": {"line": line, "character": column}}, "text": "/"
        }]));
        let edit = server.format(&uri, line, column + 1);
        assert_eq!(
            edit,
            json!([{
                "range": {"start": {"line": line, "character": column}, "end": {"line": line, "character": column + 1}}, "newText": "/>"
            }]),
            "{language_id}"
        );
        server.change(
            &uri,
            3,
            json!([{"range": edit[0]["range"], "text": edit[0]["newText"]}]),
        );
        assert_eq!(
            server.format(&uri, line, column + 1),
            json!([]),
            "{language_id}"
        );
        server.change(&uri, 4, json!([{
            "range": {"start": {"line": line, "character": column}, "end": {"line": line, "character": column + 2}}, "text": "/"
        }]));
        assert_eq!(server.format(&uri, line, column + 1), edit, "{language_id}");
        server.change(&uri, 5, json!([{
            "range": {"start": {"line": 99, "character": 0}, "end": {"line": 99, "character": 0}}, "text": "x"
        }]));
        assert_eq!(
            server.format(&uri, line, column + 1),
            json!([]),
            "{language_id}"
        );
        server.change(&uri, 6, json!([{"text": format!("{before}/{after}")}]));
        assert_eq!(server.format(&uri, line, column + 1), edit, "{language_id}");
    }
    assert!(server.finish().contains("invalid UTF-16 edit range"));
}

#[test]
fn unsupported_language_ids_do_not_fall_back_to_a_markup_grammar() {
    let (mut server, _) = Server::start();
    for language_id in ["typescript", "rust", "markdown", "unknown"] {
        server.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": URI, "languageId": language_id, "version": 1, "text": "<Component /"
            }}),
        );
        assert_eq!(server.format(URI, 0, 12), json!([]), "{language_id}");
    }
    assert!(server.finish().is_empty());
}

#[test]
fn malformed_document_notifications_suspend_only_the_identified_document() {
    let (mut server, _) = Server::start();
    let other = "file:///other.tsx";
    server.open(other, "<Component /");
    for method in ["textDocument/didOpen", "textDocument/didChange"] {
        server.open(URI, "<Component /");
        server.notify(
            method,
            json!({"textDocument": {"uri": URI, "version": "invalid"}}),
        );
        assert_eq!(server.format(URI, 0, 12), json!([]));
        assert_eq!(server.format(other, 0, 12)[0]["newText"], "/>");
        server.change(URI, 2, json!([{"text": "<Component /"}]));
        assert_eq!(server.format(URI, 0, 12)[0]["newText"], "/>");
    }
    assert!(server.finish().contains("Cannot handle"));
}

#[test]
fn unidentified_malformed_change_suspends_documents_until_reopen_or_full_replacement() {
    let (mut server, _) = Server::start();
    let other = "file:///other.tsx";
    server.open(URI, "<Component /");
    server.open(other, "<Component /");
    server.notify("textDocument/didChange", json!({"contentChanges": []}));
    assert_eq!(server.format(URI, 0, 12), json!([]));
    assert_eq!(server.format(other, 0, 12), json!([]));
    server.open(other, "<Component /");
    assert_eq!(server.format(other, 0, 12)[0]["newText"], "/>");
    server.change(URI, 2, json!([
        {"range": {"start": {"line": 99, "character": 0}, "end": {"line": 99, "character": 0}}, "text": "discarded"},
        {"text": "<Component "},
        {"range": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 11}}, "text": "/"}
    ]));
    assert_eq!(server.format(URI, 0, 12)[0]["newText"], "/>");
    assert!(
        server
            .finish()
            .contains("Cannot handle textDocument/didChange")
    );
}

#[test]
fn empty_pair_conversion_survives_client_application_and_undo() {
    let (mut server, _) = Server::start();
    server.open(URI, "<p></p>");
    server.change(URI, 2, json!([{
        "range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 2}}, "text": "/"
    }]));
    let edit = server.format(URI, 0, 3);
    assert_eq!(
        edit,
        json!([{
            "range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 8}}, "newText": " />"
        }])
    );
    server.change(
        URI,
        3,
        json!([{"range": edit[0]["range"], "text": edit[0]["newText"]}]),
    );
    assert_eq!(server.format(URI, 0, 4), json!([]));
    server.change(URI, 4, json!([{
        "range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 5}}, "text": "/></p>"
    }]));
    assert_eq!(server.format(URI, 0, 3), edit);
    assert!(server.finish().is_empty());
}
