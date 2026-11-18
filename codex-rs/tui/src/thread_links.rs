//! Markdown links to threads on the current app server. These are handled inside the TUI;
//! they must never be emitted as terminal OSC 8 links, which would launch the desktop app.

use codex_protocol::ThreadId;

pub(crate) fn thread_id(destination: &str) -> Option<ThreadId> {
    let id = destination.strip_prefix("codex://threads/")?;
    let thread_id = ThreadId::from_string(id).ok()?;
    (thread_id.to_string() == id).then_some(thread_id)
}

#[cfg(test)]
#[path = "thread_links_tests.rs"]
mod tests;
