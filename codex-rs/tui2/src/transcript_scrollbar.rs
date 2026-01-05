//! Transcript scrollbar rendering.
//!
//! The transcript in `codex-tui2` is rendered as a flattened list of wrapped visual lines. The
//! viewport is tracked as a top-row offset (`transcript_view_top`) into that flattened list (see
//! `tui/scrolling.rs` and `tui_viewport_and_history.md`).
//!
//! This module adds a scrollbar to that viewport using the `tui-scrollbar` widget, but does so in
//! a way that keeps the transcript hot path simple and avoids visual layout jank.
//!
//! # Layout and invariants
//!
//! The transcript area is split into:
//!
//! - `content_area`: where transcript text is rendered
//! - `scrollbar_area`: a 1-column region used to render the scrollbar
//!
//! Additionally, we reserve a 1-column *gap* between content and scrollbar. This produces a
//! slightly more stable/intentional look (the scrollbar reads as an affordance, not part of the
//! transcript content) and avoids accidental overlap with selection/copy UI.
//!
//! Important invariant: **any code that computes transcript wrapping, scrolling, selection, or
//! copy must use the same width as on-screen transcript rendering**. In practice that means:
//!
//! - Use [`split_transcript_area`] and pass `content_area.width` into anything that depends on
//!   transcript width (wrapping, scroll deltas, selection reconstruction for copy, etc.).
//! - Do not mix `terminal.width` and `content_area.width` for transcript operations; doing so
//!   causes off-by-one/off-by-two behaviors where the selection highlights and copied text do not
//!   match what the user sees.
//!
//! `App` follows this rule by deriving `content_area.width` anywhere it needs transcript width.
//!
//! # When the scrollbar is shown
//!
//! The scrollbar is only drawn when the transcript is *not* pinned to the bottom:
//!
//! - `offset < max_offset` → draw scrollbar
//! - `offset == max_offset` → keep the column reserved but blank
//!
//! This keeps the UI clean during normal operation (where the viewport follows streaming output),
//! while still providing a clear affordance when the user is actively reading scrollback.
//!
//! # Styling and theme heuristics
//!
//! `tui-scrollbar` 0.2.1 changed defaults (no arrows, space track + dark background). We keep
//! default glyphs/arrows so the widget controls its own shape, but override track/thumb colors so
//! it matches `codex-tui2`’s existing "user prompt block" styling:
//!
//! - The track background is a small blend toward the terminal foreground so it looks like a
//!   subtle indent against the terminal background.
//! - The thumb foreground is a stronger blend so it reads as the active element.
//! - In light themes (terminal background is light), the thumb is intentionally a *darker* shade
//!   than the track so it reads as an inset element.
//!
//! We derive these colors from the terminal’s default foreground/background (when available via
//! `terminal_palette`). When defaults are unknown (tests / unsupported terminals), we fall back to
//! ANSI colors so the scrollbar remains visible.
//!
//! # Pointer interaction (mouse click/drag)
//!
//! `tui-scrollbar` includes an interaction helper that translates pointer events into
//! `ScrollCommand::SetOffset(...)` updates, including "grab offset" handling so dragging keeps the
//! pointer anchored within the thumb.
//!
//! `codex-tui2` uses that helper rather than reimplementing scrollbar hit testing and drag math.
//! The app owns the actual transcript scroll state (anchors in `tui/scrolling.rs`), so we only use
//! `tui-scrollbar` to decide *which* offset the user requested. `App` then converts that raw
//! `offset` back into a stable [`TranscriptScroll`] anchor.
//!
//! Note: we use `tui-scrollbar`’s backend-agnostic [`ScrollEvent`] types instead of its optional
//! `crossterm` adapter, because the workspace uses a patched `crossterm` and we want to avoid
//! pulling in multiple `crossterm` versions (which would make `MouseEvent` types incompatible).
//!
//! Because the scrollbar is visually hidden while pinned-to-bottom, `App` also keeps a tiny
//! "pointer capture" bool so a drag that reaches the bottom doesn't accidentally turn into a text
//! selection once the scrollbar disappears.
//!
//! # `ratatui` vs `ratatui-core`
//!
//! `codex-tui2` uses the `ratatui` crate, while `tui-scrollbar` is built on `ratatui-core`.
//! Because the buffer and style types are distinct, we render the scrollbar into a small
//! `ratatui-core` scratch buffer and then copy the resulting glyphs into the main `ratatui`
//! buffer with `ratatui` styles.
//!
//! ## Upgrade note: Ratatui 0.30+
//!
//! Ratatui 0.30 split many core types (including `Buffer`, `Rect`, and `Widget`) into the new
//! `ratatui-core` crate. `codex-tui2` is currently pinned to an older Ratatui, so it still works
//! with `ratatui::buffer::Buffer` / `ratatui::layout::Rect`, while `tui-scrollbar` is already on
//! `ratatui-core`.
//!
//! That mismatch forces two bits of "glue" that should go away once `codex-tui2` upgrades to
//! Ratatui 0.30:
//!
//! - Rendering: `render_transcript_scrollbar_if_active` currently renders into a `ratatui-core`
//!   scratch buffer and copies glyphs/styles into the `ratatui` buffer. With Ratatui 0.30, the
//!   app’s buffer/rect types should unify with `tui-scrollbar`’s `ratatui-core` types, so we can
//!   render directly without copying.
//! - Input: we currently translate `crossterm::MouseEvent` into `tui-scrollbar`’s backend-agnostic
//!   `ScrollEvent` types (and intentionally avoid `tui-scrollbar`’s optional `crossterm` adapter)
//!   to prevent multiple `crossterm` versions in the dependency graph. Once the Ratatui upgrade
//!   is complete, this should be revisited; if the workspace’s `crossterm` resolves to a single
//!   version, we can use `tui-scrollbar`’s adapter and reduce more local glue.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui_core::buffer::Buffer as CoreBuffer;
use ratatui_core::layout::Rect as CoreRect;
use ratatui_core::widgets::Widget as _;
use tui_scrollbar::PointerButton;
use tui_scrollbar::PointerEvent;
use tui_scrollbar::PointerEventKind;
use tui_scrollbar::ScrollBar;
use tui_scrollbar::ScrollBarInteraction;
use tui_scrollbar::ScrollCommand;
use tui_scrollbar::ScrollEvent;
use tui_scrollbar::ScrollLengths;
use tui_scrollbar::TrackClickBehavior;

/// Number of columns reserved between transcript content and the scrollbar track.
///
/// This exists purely for visual separation and to avoid selection/copy UI feeling "attached" to
/// the scrollbar.
const TRANSCRIPT_SCROLLBAR_GAP_COLS: u16 = 1;
/// Width of the scrollbar track itself (in terminal cells).
///
/// `tui-scrollbar` renders a vertical scrollbar into a 1-column area.
const TRANSCRIPT_SCROLLBAR_TRACK_COLS: u16 = 1;
/// Total columns reserved for transcript scrollbar UI (gap + track).
pub(crate) const TRANSCRIPT_SCROLLBAR_COLS: u16 =
    TRANSCRIPT_SCROLLBAR_GAP_COLS + TRANSCRIPT_SCROLLBAR_TRACK_COLS;

/// Split a transcript viewport into content + scrollbar regions.
///
/// `codex-tui2` reserves space for the transcript scrollbar even when it is not visible so the
/// transcript does not "reflow" when the user scrolls away from the bottom.
///
/// Layout:
/// - `content_area`: original area minus [`TRANSCRIPT_SCROLLBAR_COLS`] on the right.
/// - `scrollbar_area`: the last column of the original area (1 cell wide).
/// - The remaining column (immediately left of `scrollbar_area`) is the "gap" and is intentionally
///   left unused so the scrollbar reads as a separate affordance.
///
/// Returns `(area, None)` when the terminal is too narrow to reserve the required columns.
pub(crate) fn split_transcript_area(area: Rect) -> (Rect, Option<Rect>) {
    if area.width <= TRANSCRIPT_SCROLLBAR_COLS {
        return (area, None);
    }

    let content_width = area.width.saturating_sub(TRANSCRIPT_SCROLLBAR_COLS);
    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: content_width,
        height: area.height,
    };
    let scrollbar_area = Rect {
        x: area.right().saturating_sub(1),
        y: area.y,
        width: TRANSCRIPT_SCROLLBAR_TRACK_COLS,
        height: area.height,
    };

    (content_area, Some(scrollbar_area))
}

/// Whether the transcript scrollbar should be visible.
///
/// The scrollbar is treated as "active" when the transcript is scrollable and the viewport is not
/// pinned to the bottom. This is used both for rendering (draw vs. keep blank) and for interaction
/// (whether the scrollbar should be hit-testable).
///
/// Note that `codex-tui2` still reserves space for the scrollbar even when it is inactive; see
/// [`split_transcript_area`].
pub(crate) fn is_transcript_scrollbar_active(
    total_lines: usize,
    viewport_lines: usize,
    top_offset: usize,
) -> bool {
    if total_lines <= viewport_lines {
        return false;
    }

    let max_offset = total_lines.saturating_sub(viewport_lines);
    top_offset < max_offset
}

/// Render the transcript scrollbar into `buf` when the viewport is scrolled away from bottom.
///
/// The scrollbar is hidden (but its column(s) remain reserved) while the viewport follows the
/// latest output.
///
/// Implementation notes:
/// - We keep `tui-scrollbar`’s default glyph selection and shape logic, but override colors to
///   better match `codex-tui2`’s theme heuristics (see module docs).
/// - Because `tui-scrollbar` renders into a `ratatui-core` buffer while `codex-tui2` uses `ratatui`
///   (pre-0.30), we render into a scratch buffer and then copy the resulting symbols into the main
///   buffer.
pub(crate) fn render_transcript_scrollbar_if_active(
    buf: &mut Buffer,
    scrollbar_area: Option<Rect>,
    total_lines: usize,
    viewport_lines: usize,
    top_offset: usize,
) {
    let Some(scrollbar_area) = scrollbar_area else {
        return;
    };

    if scrollbar_area.width == 0 || scrollbar_area.height == 0 {
        return;
    }

    if !is_transcript_scrollbar_active(total_lines, viewport_lines, top_offset) {
        return;
    }

    let lengths = ScrollLengths {
        content_len: total_lines,
        viewport_len: viewport_lines,
    };

    let scrollbar = ScrollBar::vertical(lengths).offset(top_offset);

    let core_bar_area = CoreRect {
        x: scrollbar_area.x,
        y: scrollbar_area.y,
        width: scrollbar_area.width,
        height: scrollbar_area.height,
    };
    let mut scratch = CoreBuffer::empty(core_bar_area);
    (&scrollbar).render(core_bar_area, &mut scratch);

    let (track_style, thumb_style) = scrollbar_styles();
    for row in 0..scrollbar_area.height {
        let x = scrollbar_area.x;
        let y = scrollbar_area.y + row;
        let src = &scratch[(x, y)];
        let dst = &mut buf[(x, y)];
        let symbol = src.symbol();
        dst.set_symbol(symbol);
        if symbol == " " {
            dst.set_style(track_style);
        } else {
            dst.set_style(thumb_style);
        }
    }
}

/// Convert a `crossterm` mouse event into a requested transcript offset for the scrollbar.
///
/// This is a thin wrapper over `tui-scrollbar`’s pointer interaction logic:
/// - It builds a `ScrollBar` configured with the current `top_offset`.
/// - It translates the mouse event into a backend-agnostic [`ScrollEvent`].
/// - It passes the event through `tui-scrollbar`’s hit testing and drag state (`interaction`).
///
/// `clamp_to_track` exists for `App`’s "pointer capture" behavior: once the user starts a drag on
/// the scrollbar, we keep treating the gesture as a scrollbar drag even if the pointer moves
/// outside the 1-column track. Without this clamp, the drag could stop producing offsets, and the
/// same mouse gesture could then be interpreted as transcript selection.
///
/// Returns `None` when:
/// - the scrollbar area is empty,
/// - the transcript does not scroll (`total_lines <= viewport_lines`),
/// - or the event is not a left-button down/drag/up.
pub(crate) fn transcript_scrollbar_offset_for_mouse_event(
    scrollbar_area: Rect,
    total_lines: usize,
    viewport_lines: usize,
    top_offset: usize,
    mut event: crossterm::event::MouseEvent,
    interaction: &mut ScrollBarInteraction,
    clamp_to_track: bool,
) -> Option<usize> {
    if scrollbar_area.width == 0 || scrollbar_area.height == 0 {
        return None;
    }

    if total_lines <= viewport_lines {
        return None;
    }

    if clamp_to_track {
        let max_x = scrollbar_area.right().saturating_sub(1);
        let max_y = scrollbar_area.bottom().saturating_sub(1);
        event.column = event.column.clamp(scrollbar_area.x, max_x);
        event.row = event.row.clamp(scrollbar_area.y, max_y);
    }

    let lengths = ScrollLengths {
        content_len: total_lines,
        viewport_len: viewport_lines,
    };
    let scrollbar = ScrollBar::vertical(lengths)
        .offset(top_offset)
        .track_click_behavior(TrackClickBehavior::JumpToClick);

    let core_bar_area = CoreRect {
        x: scrollbar_area.x,
        y: scrollbar_area.y,
        width: scrollbar_area.width,
        height: scrollbar_area.height,
    };
    let scroll_event = match event.kind {
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            Some(ScrollEvent::Pointer(PointerEvent {
                column: event.column,
                row: event.row,
                kind: PointerEventKind::Down,
                button: PointerButton::Primary,
            }))
        }
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            Some(ScrollEvent::Pointer(PointerEvent {
                column: event.column,
                row: event.row,
                kind: PointerEventKind::Up,
                button: PointerButton::Primary,
            }))
        }
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            Some(ScrollEvent::Pointer(PointerEvent {
                column: event.column,
                row: event.row,
                kind: PointerEventKind::Drag,
                button: PointerButton::Primary,
            }))
        }
        _ => None,
    };
    scroll_event
        .and_then(|scroll_event| scrollbar.handle_event(core_bar_area, scroll_event, interaction))
        .map(|command| match command {
            ScrollCommand::SetOffset(offset) => offset,
        })
}

/// Derive track/thumb styles for the scrollbar from terminal defaults.
///
/// We prefer using the terminal’s default background/foreground so the scrollbar feels like a
/// native part of the theme (and stays readable across 16-color / 256-color / truecolor
/// backends).
///
/// When terminal defaults are unavailable (tests / unsupported terminals), we fall back to fixed
/// ANSI colors that are likely to be visible.
fn scrollbar_styles() -> (Style, Style) {
    let Some(terminal_bg) = crate::terminal_palette::default_bg() else {
        let track_style = Style::new().bg(Color::DarkGray);
        let thumb_style = Style::new().fg(Color::Gray).bg(Color::DarkGray);
        return (track_style, thumb_style);
    };

    let terminal_fg = crate::terminal_palette::default_fg();

    let (track_rgb, thumb_rgb) = scrollbar_colors(terminal_bg, terminal_fg);
    let track_bg = crate::terminal_palette::best_color(track_rgb);
    let thumb_fg = crate::terminal_palette::best_color(thumb_rgb);

    let track_style = Style::new().bg(track_bg);
    let thumb_style = Style::new().fg(thumb_fg).bg(track_bg);
    (track_style, thumb_style)
}

/// Compute `(track_bg_rgb, thumb_fg_rgb)` for the transcript scrollbar.
///
/// The scrollbar is styled to feel consistent with the user prompt background (see
/// `style::user_message_bg`), but is tuned separately so the thumb reads as an inset control:
///
/// - Dark themes: track is a subtle brightening of the background; thumb is brighter than the track
///   (but not pure white).
/// - Light themes: track is a subtle darkening of the background; thumb is darker than the track.
fn scrollbar_colors(
    terminal_bg: (u8, u8, u8),
    terminal_fg: Option<(u8, u8, u8)>,
) -> ((u8, u8, u8), (u8, u8, u8)) {
    let is_light = crate::color::is_light(terminal_bg);
    let fallback_fg = if is_light { (0, 0, 0) } else { (255, 255, 255) };
    let terminal_fg = terminal_fg.unwrap_or(fallback_fg);

    // We want the scrollbar to feel visually related to the user message block background
    // (`style::user_message_bg` uses 0.1), but slightly more subtle:
    //
    // - Light mode: keep both colors closer to the background (alpha < 0.1), with the thumb darker
    //   than the track.
    // - Dark mode: keep the track slightly darker than the prompt block, but make the thumb
    //   brighter so it's easy to pick out without becoming "white".
    let (track_alpha, thumb_alpha) = if is_light { (0.04, 0.08) } else { (0.08, 0.18) };

    let track_rgb = crate::color::blend(terminal_fg, terminal_bg, track_alpha);
    let thumb_rgb = crate::color::blend(terminal_fg, terminal_bg, thumb_alpha);
    (track_rgb, thumb_rgb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn scrollbar_bg(buf: &Buffer, scrollbar_area: Rect) -> Vec<ratatui::style::Color> {
        use ratatui::style::Color;

        let x = scrollbar_area.x;
        (0..scrollbar_area.height)
            .map(|row| {
                buf[(x, scrollbar_area.y + row)]
                    .style()
                    .bg
                    .unwrap_or(Color::Reset)
            })
            .collect()
    }

    #[test]
    fn does_not_render_when_pinned_to_bottom() {
        let full_area = Rect::new(0, 0, 10, 6);
        let (_, scrollbar_area) = split_transcript_area(full_area);
        let mut buf = Buffer::empty(full_area);

        render_transcript_scrollbar_if_active(&mut buf, scrollbar_area, 100, 6, 94);

        assert_eq!(
            scrollbar_bg(&buf, scrollbar_area.expect("scrollbar area")),
            vec![
                ratatui::style::Color::Reset,
                ratatui::style::Color::Reset,
                ratatui::style::Color::Reset,
                ratatui::style::Color::Reset,
                ratatui::style::Color::Reset,
                ratatui::style::Color::Reset
            ]
        );
    }

    #[test]
    fn renders_when_scrolled_away_from_bottom() {
        let full_area = Rect::new(0, 0, 10, 6);
        let (_, scrollbar_area) = split_transcript_area(full_area);
        let mut buf = Buffer::empty(full_area);

        render_transcript_scrollbar_if_active(&mut buf, scrollbar_area, 100, 6, 80);

        assert_eq!(
            scrollbar_bg(&buf, scrollbar_area.expect("scrollbar area")),
            vec![
                ratatui::style::Color::DarkGray,
                ratatui::style::Color::DarkGray,
                ratatui::style::Color::DarkGray,
                ratatui::style::Color::DarkGray,
                ratatui::style::Color::DarkGray,
                ratatui::style::Color::DarkGray
            ]
        );
    }

    #[test]
    fn split_leaves_gap_before_scrollbar() {
        let full_area = Rect::new(0, 0, 10, 6);
        let (content, scrollbar) = split_transcript_area(full_area);

        assert_eq!(content.width, 8);
        assert_eq!(scrollbar.expect("scrollbar").x, 9);
    }

    #[test]
    fn scrollbar_mouse_drag_moves_offset_downward() {
        use crossterm::event::KeyModifiers;
        use crossterm::event::MouseButton;
        use crossterm::event::MouseEvent;
        use crossterm::event::MouseEventKind;

        let scrollbar_area = Rect::new(9, 0, 1, 10);
        let mut interaction = ScrollBarInteraction::new();

        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 9,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let mut offset = 0;
        if let Some(next) = transcript_scrollbar_offset_for_mouse_event(
            scrollbar_area,
            100,
            10,
            offset,
            down,
            &mut interaction,
            true,
        ) {
            offset = next;
        }

        let drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 9,
            row: 9,
            modifiers: KeyModifiers::empty(),
        };
        let dragged = transcript_scrollbar_offset_for_mouse_event(
            scrollbar_area,
            100,
            10,
            offset,
            drag,
            &mut interaction,
            true,
        )
        .expect("drag should set an offset");

        assert!(dragged > offset);
    }

    #[test]
    fn light_mode_thumb_is_darker_than_track() {
        let bg = (255, 255, 255);
        let fg = Some((0, 0, 0));
        let (track, thumb) = scrollbar_colors(bg, fg);

        assert!(thumb.0 < track.0);
        assert!(thumb.1 < track.1);
        assert!(thumb.2 < track.2);
    }

    #[test]
    fn dark_mode_thumb_is_brighter_than_track() {
        let bg = (0, 0, 0);
        let fg = Some((255, 255, 255));
        let (track, thumb) = scrollbar_colors(bg, fg);

        assert!(thumb.0 > track.0);
        assert!(thumb.1 > track.1);
        assert!(thumb.2 > track.2);
    }
}
