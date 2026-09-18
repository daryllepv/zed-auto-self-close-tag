//! Reproducible release-mode timings for incremental TSX editing and completion.
use std::{hint::black_box, time::Instant};

use auto_self_close_tag_lsp::{Document, Language};
use lsp_types::{Position, Range, TextDocumentContentChangeEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("scenario,bytes,open_ms,typing_median_us,typing_p95_us,slash_median_us,slash_p95_us");
    let row = "    <Component title=\"some label\" value={total / 2} />\n";
    for (scenario, suffix) in [("delimiter", ""), ("pair", "></Component>")] {
        for size in [10_000, 100_000, 1_000_000] {
            let rows = size / row.len();
            let text = format!(
                "const view = <>\n{}    <Component {suffix}\n</>;",
                row.repeat(rows)
            );
            let before = Position::new(rows as u32 + 1, 15);
            let after = Position::new(before.line, before.character + 1);
            let start = Instant::now();
            let mut document = Document::new(Language::Tsx, &text)?;
            let open_ms = start.elapsed().as_secs_f64() * 1000.0;
            let mut typing = Vec::new();
            let mut closing = Vec::new();
            for index in 0..110 {
                for (ch, samples) in [("x", &mut typing), ("/", &mut closing)] {
                    let start = Instant::now();
                    document.change(&[TextDocumentContentChangeEvent {
                        range: Some(Range::new(before, before)),
                        range_length: None,
                        text: ch.into(),
                    }])?;
                    if ch == "/" {
                        assert!(black_box(document.complete(after)?).is_some());
                    }
                    if index >= 10 {
                        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0);
                    }
                    document.change(&[TextDocumentContentChangeEvent {
                        range: Some(Range::new(before, after)),
                        range_length: None,
                        text: String::new(),
                    }])?;
                }
            }
            typing.sort_by(f64::total_cmp);
            closing.sort_by(f64::total_cmp);
            println!(
                "{scenario},{},{:.2},{:.1},{:.1},{:.1},{:.1}",
                text.len(),
                open_ms,
                typing[50],
                typing[95],
                closing[50],
                closing[95]
            );
        }
    }
    Ok(())
}
