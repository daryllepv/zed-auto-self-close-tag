# Development

## Architecture

| File | Responsibility |
| --- | --- |
| `src/lib.rs` | Locate and launch the native server through Zed's API. |
| `server/src/main.rs` | LSP transport, initialization, requests, and shutdown. |
| `server/src/documents.rs` | Open documents, language selection, versions, and synchronization recovery. |
| `server/src/lib.rs` | Atomic buffer edits, UTF-16 positions, incremental parsing, and candidate edits. |
| `server/src/languages.rs` | Supported language IDs, grammar selection, and validation dispatch. |
| `server/src/languages/jsx.rs` | Shared JSX/TSX syntax validation, using each language's own grammar. |
| `server/src/languages/{astro,svelte,vue,html,xml}.rs` | Format-specific context and self-closing rules. |
| `server/src/languages/pairs.rs` | Grammar-specific matching of empty opening/closing tag pairs. |
| `server/src/languages/markup.rs` | Shared HTML-derived node traversal and conservative recovery checks. |

Each document has an explicit `Language`. The lifecycle manager retains that
language even when invalid changes suspend its contents, so a full replacement
uses the original grammar. Unsupported language IDs never fall back to TSX.

The engine proposes inserting `>` after `/`, parses the candidate, then asks the
language's validator whether the resulting delimiter may self-close. Validation
must use that grammar's syntax and the language's semantics. Buffer updates,
position conversion, and the returned slash-replacement edit are shared.

When `>` already follows the slash, the engine reconstructs the paired syntax,
checks for adjacent tags with matching names, and validates the self-closing
replacement with the same language rules. Nonempty elements are left unchanged.

## Adding a language

1. Add the Tree-sitter grammar dependency to `server/Cargo.toml`.
2. Add a variant to `Language` in `server/src/languages.rs`. Map the language's
   actual LSP identifiers in `from_language_id`, select its grammar in `grammar`,
   and dispatch `allows_self_close` to a language module.
3. Add `server/src/languages/<language>.rs`. Its validator receives the candidate
   tree, candidate Rope, and slash's byte offset. Check the exact delimiter, its
   owning syntax construct, parse errors, and whether the construct permits
   self-closing. Read only relevant source slices. Additional parsing can use the
   shared RopeSlice parser, as Astro does for template expressions.
4. Add the existing Zed language name and its explicit `language_ids` mapping to
   `extension.toml` and document required language-server settings. Zed language
   names and LSP IDs are separate values; Vue uses `Vue.js` and `vue` respectively.
5. Add completion fixtures covering accepted syntax, existing delimiters,
   comments, strings, expressions, malformed syntax, and embedded-language
   boundaries. Verify final edited text, not just the presence of an edit.
6. Exercise its LSP ID through open, completion, invalid-change suspension, and
   full-replacement recovery in `server/tests/lsp.rs`.
7. Run the checks below and manually verify the language in Zed.

TSX, JSX, Astro, Svelte, Vue, HTML, and XML are supported. A language with embedded
scripts needs context validation. HTML distinguishes void elements from ordinary
elements and has SVG/MathML integration points; recognizing `/>` alone does not
establish correctness. Svelte's policy also excludes ordinary non-void HTML tags.
The shared engine covers slash-triggered `/>` completion, not arbitrary closing
tag insertion or other trigger characters.

## Checks

```sh
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build -p auto-self-close-tag --target wasm32-wasip2 --locked
cargo bench -p auto-self-close-tag-lsp --bench typing --locked
```

Install `wasm32-wasip2` through rustup before the Wasm build. CI runs native tests,
formatting, and Clippy on Linux, macOS, and Windows, plus a release Wasm build.
Benchmarks report timings rather than enforcing machine-dependent thresholds.

Tests should protect distinct behavior. Keep language syntax fixtures together;
use the real stdio server for protocol and lifecycle behavior. Editor cursor
placement and undo grouping require manual Zed checks.

## Grammar dependencies

The server uses Tree-sitter 0.25 with the following grammar crates:

| Format | Crate | Version series |
| --- | --- | --- |
| TSX | `tree-sitter-typescript` | 0.23 |
| JSX | `tree-sitter-javascript` | 0.25 |
| Astro | `tree-sitter-astro-next` | 0.1.1 |
| Svelte | `tree-sitter-svelte-ng` | 1.0.2 |
| Vue | `tree-sitter-vue-next` | 0.1 |
| HTML | `tree-sitter-html` | 0.23 |
| XML | `tree-sitter-xml` | 0.7 |

The Astro and Vue crates provide compatible Rust bindings for those grammars.
Keep `Cargo.lock` in version control. When upgrading a grammar, run the syntax and
stdio lifecycle tests; parsing an isolated complete tag is not sufficient.

The markup grammars may recover comments, declarations, or interpolations as
ordinary tags. The engine rejects candidates after unresolved parse errors to
avoid false positives. The XML scanner also accepts some invalid initial name
characters, so its validator checks those. Its non-ASCII name recognition remains
limited by the scanner. These limitations should be revisited with grammar updates.

See Zed's [publishing requirements](https://zed.dev/docs/extensions/publishing/prerequisites)
before submitting a release. Manual checks must cover every enabled language on
the exact submission commit.
