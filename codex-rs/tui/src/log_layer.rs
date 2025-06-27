//! Custom `tracing_subscriber` layer that forwards every formatted log event to the
//! TUI so the status indicator can display the *latest* log line while a task is
//! running.
//!
//! The layer is intentionally extremely small: we implement `on_event()` only and
//! ignore spans/metadata because we only care about the already‑formatted output
//! that the default `fmt` layer would print.  We therefore borrow the same
//! formatter (`tracing_subscriber::fmt::format::FmtSpan`) used by the default
//! fmt layer so the text matches what is written to the log file.

use std::fmt::Write as _;

use tokio::sync::mpsc::UnboundedSender;
use tracing::Event;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// Maximum characters forwarded to the TUI. Longer messages are truncated so the
/// single‑line status indicator cannot overflow the viewport.
#[allow(dead_code)]
const _DEFAULT_MAX_LEN: usize = 120;

pub struct TuiLogLayer {
    tx: UnboundedSender<String>,
    max_len: usize,
}

impl TuiLogLayer {
    pub fn new(tx: UnboundedSender<String>, max_len: usize) -> Self {
        Self {
            tx,
            max_len: max_len.max(8),
        }
    }
}

impl<S> Layer<S> for TuiLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Build a terse line like `[TRACE core::session] message …` by visiting
        // fields into a buffer. This avoids pulling in the heavyweight
        // formatter machinery.

        struct Visitor<'a> {
            buf: &'a mut String,
        }

        impl Visit for Visitor<'_> {
            fn record_debug(&mut self, _field: &Field, value: &dyn std::fmt::Debug) {
                let _ = write!(self.buf, " {:?}", value);
            }
        }

        let mut buf = String::new();
        let _ = write!(
            buf,
            "[{} {}]",
            event.metadata().level(),
            event.metadata().target()
        );

        event.record(&mut Visitor { buf: &mut buf });

        // `String::truncate` operates on UTF‑8 code‑point boundaries and will
        // panic if the provided index is not one.  Because we limit the log
        // line by its **byte** length we can not guarantee that the index we
        // want to cut at happens to be on a boundary.  Therefore we fall back
        // to a simple, boundary‑safe loop that pops complete characters until
        // the string is within the designated size.

        if buf.len() > self.max_len {
            // Attempt direct truncate at the byte index.  If that is not a
            // valid boundary we advance to the next one ( ≤3 bytes away ).
            if buf.is_char_boundary(self.max_len) {
                buf.truncate(self.max_len);
            } else {
                let mut idx = self.max_len;
                while idx < buf.len() && !buf.is_char_boundary(idx) {
                    idx += 1;
                }
                buf.truncate(idx);
            }
        }

        let sanitized = buf.replace(['\n', '\r'], " ");
        let _ = self.tx.send(sanitized);
    }
}
