//! Zed extension that launches the locally installed tag-closing server.

use zed_extension_api as zed;

struct AutoSelfCloseTag;

impl zed::Extension for AutoSelfCloseTag {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let command = worktree.which("auto-self-close-tag-lsp").ok_or_else(|| {
            "Build the companion server with `cargo build --release -p auto-self-close-tag-lsp` and set \
             lsp.auto-self-close-tag.binary.path to its absolute path in Zed settings."
                .to_string()
        })?;
        Ok(zed::Command {
            command,
            args: vec![],
            env: vec![],
        })
    }
}

// Keep Zed's generated entry point out of the Rust public API.
mod registration {
    use super::*;
    zed::register_extension!(AutoSelfCloseTag);
}
