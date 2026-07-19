//! Single-width render cache shared by finalized source-backed markdown history cells.

use crate::terminal_hyperlinks::HyperlinkLine;
use std::sync::Mutex;
use std::sync::PoisonError;

#[derive(Debug, Default)]
pub(super) struct MarkdownRenderCache {
    pub(super) cached: Mutex<Option<(MarkdownRenderCacheKey, Vec<HyperlinkLine>)>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MarkdownRenderCacheKey {
    pub(super) width: u16,
    pub(super) syntax_theme_revision: u64,
    pub(super) terminal_fg: Option<(u8, u8, u8)>,
    pub(super) terminal_bg: Option<(u8, u8, u8)>,
    pub(super) color_level: crate::terminal_palette::StdoutColorLevel,
}

impl MarkdownRenderCache {
    /// Return lines cached for this width and terminal render state, rendering on a cache miss.
    ///
    /// Only the most recent entry is retained, so changing width, syntax theme, or terminal colors
    /// replaces the cached render.
    pub(super) fn render(
        &self,
        width: u16,
        render: impl FnOnce() -> Vec<HyperlinkLine>,
    ) -> Vec<HyperlinkLine> {
        let key = MarkdownRenderCacheKey {
            width,
            syntax_theme_revision: crate::render::highlight::syntax_theme_revision(),
            terminal_fg: crate::terminal_palette::default_fg(),
            terminal_bg: crate::terminal_palette::default_bg(),
            color_level: crate::terminal_palette::stdout_color_level(),
        };
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((cached_key, lines)) = cached.as_ref()
            && *cached_key == key
        {
            return lines.clone();
        }

        let lines = render();
        *cached = Some((key, lines.clone()));
        lines
    }
}
