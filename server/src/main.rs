//! Stdio LSP server for completing self-closing tags.

mod documents;

use std::error::Error;

use lsp_server::{Connection, ErrorCode, Message, Response};
use lsp_types::DocumentOnTypeFormattingParams;
use serde_json::json;

use documents::Documents;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let (connection, threads) = Connection::stdio();
    connection.initialize(json!({
        "positionEncoding": "utf-16",
        "textDocumentSync": { "openClose": true, "change": 2 },
        "documentOnTypeFormattingProvider": { "firstTriggerCharacter": "/" }
    }))?;
    let mut documents = Documents::default();

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }
                let response = if request.method == "textDocument/onTypeFormatting" {
                    match serde_json::from_value::<DocumentOnTypeFormattingParams>(request.params) {
                        Ok(params) => match documents.complete(params) {
                            Ok(edit) => {
                                Response::new_ok(request.id, edit.into_iter().collect::<Vec<_>>())
                            }
                            Err(error) => Response::new_err(
                                request.id,
                                ErrorCode::InternalError as i32,
                                error.to_string(),
                            ),
                        },
                        Err(error) => Response::new_err(
                            request.id,
                            ErrorCode::InvalidParams as i32,
                            error.to_string(),
                        ),
                    }
                } else {
                    Response::new_err(
                        request.id,
                        ErrorCode::MethodNotFound as i32,
                        format!("Unsupported request: {}", request.method),
                    )
                };
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(notification) => {
                if notification.method == "exit" {
                    std::process::exit(1);
                }
                documents.notify(notification);
            }
            Message::Response(_) => {}
        }
    }
    drop(connection);
    threads.join()?;
    Ok(())
}
