use std::{collections::HashMap, error::Error as StdError};

use auto_self_close_tag_lsp::{Document, Error, Language};
use lsp_server::Notification;
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentOnTypeFormattingParams, TextEdit, Url,
};

struct OpenDocument {
    language: Language,
    version: i32,
    contents: Option<Document>,
}

#[derive(Default)]
pub(super) struct Documents {
    open: HashMap<Url, OpenDocument>,
}

impl Documents {
    pub(super) fn complete(
        &mut self,
        params: DocumentOnTypeFormattingParams,
    ) -> Result<Option<TextEdit>, Error> {
        if params.ch != "/" {
            return Ok(None);
        }
        let Some(document) = self
            .open
            .get_mut(&params.text_document_position.text_document.uri)
            .and_then(|doc| doc.contents.as_mut())
        else {
            return Ok(None);
        };
        document.complete(params.text_document_position.position)
    }

    pub(super) fn notify(&mut self, notification: Notification) {
        let uri = notification
            .params
            .pointer("/textDocument/uri")
            .and_then(|value| value.as_str())
            .and_then(|value| Url::parse(value).ok());
        let version = notification
            .params
            .pointer("/textDocument/version")
            .and_then(|value| value.as_i64())
            .and_then(|value| i32::try_from(value).ok());
        let method = notification.method.clone();
        if let Err(error) = self.handle_notification(notification) {
            eprintln!(
                "Cannot handle {method}: {error}; reopen the document or send a full replacement"
            );
            if let Some(uri) = uri {
                if let Some(doc) = self.open.get_mut(&uri) {
                    doc.contents = None;
                    if let Some(version) = version {
                        doc.version = doc.version.max(version);
                    }
                }
            } else {
                // An unidentified change may affect any cached document. Keep
                // the session alive, but never format potentially stale text.
                for doc in self.open.values_mut() {
                    doc.contents = None;
                }
            }
        }
    }

    fn handle_notification(
        &mut self,
        notification: Notification,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match notification.method.as_str() {
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let doc = params.text_document;
                if let Some(language) = Language::from_language_id(&doc.language_id) {
                    self.open.insert(
                        doc.uri,
                        OpenDocument {
                            language,
                            version: doc.version,
                            contents: Some(Document::new(language, &doc.text)?),
                        },
                    );
                } else {
                    self.open.remove(&doc.uri);
                }
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                self.change(params)?;
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                self.open.remove(&params.text_document.uri);
            }
            _ => {}
        }
        Ok(())
    }

    fn change(&mut self, params: DidChangeTextDocumentParams) -> Result<(), Error> {
        let Some(doc) = self.open.get_mut(&params.text_document.uri) else {
            return Ok(());
        };
        if params.text_document.version <= doc.version {
            return Ok(());
        }
        doc.version = params.text_document.version;
        if let Some(contents) = &mut doc.contents {
            contents.change(&params.content_changes)?;
        } else if let Some(index) = params
            .content_changes
            .iter()
            .rposition(|change| change.range.is_none())
        {
            // A full replacement supersedes all preceding changes in the batch.
            let mut contents = Document::new(doc.language, &params.content_changes[index].text)?;
            contents.change(&params.content_changes[index + 1..])?;
            doc.contents = Some(contents);
        }
        Ok(())
    }
}
