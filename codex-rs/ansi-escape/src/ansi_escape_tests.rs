//! Check that multiline warnings omit payloads and preserve first-line rendering.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::text::Line;
use tracing::Event;
use tracing::Level;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;

use super::ansi_escape_line;

#[derive(Debug, PartialEq, Eq)]
struct CapturedEvent {
    level: Level,
    fields: BTreeMap<String, String>,
}

impl Visit for CapturedEvent {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

struct CaptureLayer(Arc<Mutex<Vec<CapturedEvent>>>);

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut captured = CapturedEvent {
            level: *event.metadata().level(),
            fields: BTreeMap::new(),
        };
        event.record(&mut captured);
        self.0.lock().expect("capture events").push(captured);
    }
}

#[test]
fn multiline_warning_contains_counts_and_preserves_the_styled_first_line() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(CaptureLayer(Arc::clone(&events)));
    let mut expected_events = Vec::new();

    tracing::subscriber::with_default(subscriber, || {
        for tail_bytes in [8, 128 * 1024] {
            let tail = "x".repeat(tail_bytes);
            let input = format!("\x1b[31mfirst\x1b[0m\n{tail}\nlast");
            assert_eq!(ansi_escape_line(&input), Line::from("first".red()));
            expected_events.push(CapturedEvent {
                level: Level::WARN,
                fields: BTreeMap::from([
                    ("input_bytes".to_string(), input.len().to_string()),
                    ("line_count".to_string(), "3".to_string()),
                    (
                        "message".to_string(),
                        "ansi_escape_line: expected a single line".to_string(),
                    ),
                ]),
            });
        }
    });

    assert_eq!(
        *events.lock().expect("read captured events"),
        expected_events
    );
}
