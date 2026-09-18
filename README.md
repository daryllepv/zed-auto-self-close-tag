# Auto Self-close Tag

Zed extension that automatically complete self-closing tags in Zed by typing `/`. Supports TSX, JSX,
Astro, Svelte, Vue, HTML, and XML.

## Usage

Type `/` at the end of an opening tag to complete it:

```text
<Component → <Component/>
```

For an empty tag pair, type `/` before the opening tag’s `>` to replace the pair with a self-closing tag:

```text
<p></p> → <p />
```

Completion is syntax-aware: slashes in comments, strings, attribute values, and embedded scripts or styles do not trigger it.

## Supported languages

| Language | Tags that can self-close |
| --- | --- |
| TSX / JSX | Elements and components, including member names such as `UI.Button`. |
| Astro | Template elements and components, including tags in template expressions. |
| Svelte | Components, special Svelte elements, slots, HTML void elements, and SVG/MathML elements where allowed. |
| Vue | Elements and components in HTML templates. |
| HTML | Void elements such as `img` and `input`, and SVG/MathML elements where allowed. |
| XML | Elements, including namespace-prefixed names. |

The `<p />` example works in TSX, JSX, Astro, Vue templates, and XML. Ordinary non-void HTML elements such as `p` and `div` are excluded in HTML and Svelte.

## Local installation

You’ll need Zed and Rust installed through rustup. Install the Zed language extensions for Astro, Svelte, Vue, or XML if you use those formats.

1. Clone this repository and build the companion language server:

   ```sh
   git clone https://github.com/daryllepv/auto-self-close-tag.git
   cd auto-self-close-tag
   cargo build --release -p auto-self-close-tag-lsp --locked
   ```

2. Run `zed: install dev extension` from Zed’s command palette and select the
   repository directory.

3. Merge the settings below into your Zed settings. Replace the binary path with
   the absolute path to your build and keep the language entries you use.

```json
{
  "lsp": {
    "auto-self-close-tag": {
      "binary": {
        "path": "/absolute/path/to/auto-self-close-tag/target/release/auto-self-close-tag-lsp"
      }
    }
  },
  "languages": {
    "TSX": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "JavaScript": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "Astro": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "Svelte": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "Vue.js": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "HTML": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    },
    "XML": {
      "language_servers": ["auto-self-close-tag", "..."],
      "use_on_type_format": true
    }
  }
}
```

`"..."` retains your other language servers. If you already have a custom `language_servers` list, add `"auto-self-close-tag"` to the beginning of it. The `JavaScript` entry also covers JSX files; Vue’s Zed language name is `Vue.js`.

On Windows, use the server’s `.exe` path and escape backslashes in JSON. Alternatively, put `auto-self-close-tag-lsp` on Zed’s `PATH` and omit the binary path setting.

## Troubleshooting

If typing `/` does nothing, check that:

- Zed recognizes the file as one of the supported languages.
- `auto-self-close-tag` is enabled for that language and `use_on_type_format` is `true`.
- The configured binary path points to the server you built.
- The tag can self-close in that language. Pair conversion also requires an empty, matching pair.
