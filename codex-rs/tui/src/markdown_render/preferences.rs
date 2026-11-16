//! Content preferences shared by streaming and completed Markdown renderers.
//!
//! Seed before startup previews, then refresh when resolved session settings become active.
//! Tests isolate preferences per thread so parallel renderer tests cannot affect one another.

use codex_config::types::TuiRendering;

#[cfg(not(test))]
static RENDERING: std::sync::LazyLock<std::sync::RwLock<TuiRendering>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(TuiRendering::default()));

#[cfg(test)]
thread_local! {
    static RENDERING: std::cell::Cell<TuiRendering> = std::cell::Cell::new(TuiRendering::default());
}

pub(crate) fn init(rendering: TuiRendering) {
    #[cfg(not(test))]
    {
        *RENDERING
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = rendering;
    }
    #[cfg(test)]
    RENDERING.set(rendering);
}

pub(crate) fn current() -> TuiRendering {
    #[cfg(not(test))]
    {
        *RENDERING
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    #[cfg(test)]
    {
        RENDERING.get()
    }
}

#[cfg(test)]
#[path = "preferences_tests.rs"]
mod tests;
// Shared by the streaming and replay renderers for this TUI session.
#[cfg(not(test))]
static DOCUMENT_LINKS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
thread_local! {
    static DOCUMENT_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn set_document_links(enabled: bool) {
    #[cfg(not(test))]
    DOCUMENT_LINKS.store(enabled, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    DOCUMENT_LINKS.set(enabled);
}

thread_local! {
    static SHOW_LOCAL_PATHS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn with_visible_local_paths<T>(render: impl FnOnce() -> T) -> T {
    struct Restore<'a>(&'a std::cell::Cell<bool>, bool);

    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.set(self.1);
        }
    }

    SHOW_LOCAL_PATHS.with(|show| {
        let _restore = Restore(show, show.replace(true));
        render()
    })
}

pub(crate) fn hide_transcript_destination(destination: &str) -> bool {
    if document_links_enabled() {
        super::hide_reader_link_destination(destination)
    } else {
        super::hide_web_link_destination(destination)
    }
}

pub(crate) fn document_links_enabled() -> bool {
    #[cfg(not(test))]
    let documents = DOCUMENT_LINKS.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    let documents = DOCUMENT_LINKS.get();
    documents && !SHOW_LOCAL_PATHS.get()
}
