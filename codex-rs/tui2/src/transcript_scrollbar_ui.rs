//! Transcript scrollbar mouse interaction.
//!
//! This module handles pointer interaction (click/drag) for the transcript scrollbar rendered by
//! [`crate::transcript_scrollbar`]. It exists to keep `app.rs` from growing further: the transcript
//! is a particularly stateful part of the UI (selection, wrapping, scroll anchoring, copy, etc.),
//! and scrollbar interaction needs to coordinate with several of those subsystems.
//!
//! # Responsibilities
//!
//! - Translate `crossterm` mouse events into `tui-scrollbar` interaction events (backend-agnostic
//!   [`tui_scrollbar::ScrollEvent`]).
//! - Maintain `tui-scrollbar`’s drag interaction state (`ScrollBarInteraction`) across frames so
//!   the thumb "grab offset" behaves naturally.
//! - Maintain a tiny "pointer capture" flag so a drag that reaches the bottom doesn't fall through
//!   into transcript selection once the scrollbar becomes visually hidden (because the view is now
//!   pinned to bottom).
//!
//! This module does *not* render anything. Rendering lives in `transcript_scrollbar.rs`.
//!
//! # Interaction model and transcript anchors
//!
//! `tui-scrollbar` reports requested scroll positions as a raw `offset` (a top-row index). The
//! transcript scroll state in `codex-tui2` is represented as a stable anchor
//! ([`crate::tui::scrolling::TranscriptScroll`]) so it survives transcript growth and re-wrapping.
//!
//! The conversion happens here:
//! - Ask `tui-scrollbar` for a `next_offset`.
//! - Convert that concrete offset back into a stable anchor using
//!   [`crate::tui::scrolling::TranscriptScroll::anchor_for`].
//! - If the requested offset is the bottom-most valid position, use `ToBottom` rather than a fixed
//!   anchor, restoring auto-follow behavior.
//!
//! This keeps scrollbar interaction consistent with other scroll mechanisms (wheel, PgUp/PgDn),
//! which also operate in terms of the `TranscriptScroll` state machine.
//!
//! # Upgrade note: Ratatui 0.30+
//!
//! This module intentionally uses `tui-scrollbar`’s backend-agnostic event types instead of its
//! optional `crossterm` adapter. The workspace uses a patched `crossterm`, and enabling the adapter
//! would pull in a second `crossterm` version, making `MouseEvent` types incompatible.
//!
//! Once `codex-tui2` upgrades to Ratatui 0.30 (and the workspace converges on a single `crossterm`
//! version), we should revisit whether we can remove this translation layer.

use crate::history_cell::HistoryCell;
use crate::transcript_scrollbar::is_transcript_scrollbar_active;
use crate::transcript_scrollbar::transcript_scrollbar_offset_for_mouse_event;
use crate::transcript_view_cache::TranscriptViewCache;
use crate::tui;
use crate::tui::scrolling::MouseScrollState;
use crate::tui::scrolling::TranscriptScroll;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Rect;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptScrollbarMouseHandling {
    /// The event is unrelated to the scrollbar; callers may handle it normally (e.g. selection).
    NotHandled,
    /// The event was handled by the scrollbar logic and should not be interpreted as selection.
    Handled,
}

/// Persistent UI state for transcript scrollbar pointer interaction.
///
/// This stores `tui-scrollbar`’s drag state (`ScrollBarInteraction`) plus a small "pointer capture"
/// flag used by `codex-tui2`:
///
/// - When the user clicks the scrollbar thumb/track, we enter pointer capture.
/// - While capture is active, subsequent drag events are treated as scrollbar drags even if the
///   pointer leaves the 1-column track.
/// - Capture is released on `MouseUp`.
///
/// The capture flag is important because the transcript scrollbar is hidden while pinned to
/// bottom; without capture, a drag that reaches the bottom could stop producing offsets and fall
/// through into transcript selection mid-gesture.
#[derive(Debug, Default)]
pub(crate) struct TranscriptScrollbarUi {
    interaction: tui_scrollbar::ScrollBarInteraction,
    pointer_capture: bool,
}

/// Bundles the arguments needed to handle a transcript scrollbar mouse event.
///
/// This is intentionally a struct (rather than a long argument list) because scrollbar interaction
/// touches several pieces of transcript state at once: wrapping cache, scroll anchor state, the
/// concrete top-row offset, and the wheel-scroll stream state machine. Grouping them makes call
/// sites easier to scan and helps keep `app.rs` glue minimal.
pub(crate) struct TranscriptScrollbarMouseEvent<'a> {
    pub(crate) tui: &'a mut tui::Tui,
    pub(crate) mouse_event: MouseEvent,
    pub(crate) transcript_area: Rect,
    pub(crate) scrollbar_area: Option<Rect>,
    pub(crate) transcript_cells: &'a [Arc<dyn HistoryCell>],
    pub(crate) transcript_view_cache: &'a mut TranscriptViewCache,
    pub(crate) transcript_scroll: &'a mut TranscriptScroll,
    pub(crate) transcript_view_top: &'a mut usize,
    pub(crate) transcript_total_lines: &'a mut usize,
    pub(crate) mouse_scroll_state: &'a mut MouseScrollState,
}

impl TranscriptScrollbarUi {
    pub(crate) fn pointer_capture_active(&self) -> bool {
        self.pointer_capture
    }

    /// Handle click/drag events for the transcript scrollbar.
    ///
    /// The caller is expected to provide the transcript layout for the current terminal size:
    /// `transcript_area` for content and `scrollbar_area` for the 1-column scrollbar track. See
    /// [`crate::transcript_scrollbar::split_transcript_area`].
    ///
    /// Returns [`TranscriptScrollbarMouseHandling::Handled`] when the event should not be
    /// interpreted as transcript selection (either because it updated the scroll position or
    /// because an in-progress scrollbar drag is being captured).
    pub(crate) fn handle_mouse_event(
        &mut self,
        event: TranscriptScrollbarMouseEvent<'_>,
    ) -> TranscriptScrollbarMouseHandling {
        let TranscriptScrollbarMouseEvent {
            tui,
            mouse_event,
            transcript_area,
            scrollbar_area,
            transcript_cells,
            transcript_view_cache,
            transcript_scroll,
            transcript_view_top,
            transcript_total_lines,
            mouse_scroll_state,
        } = event;
        let is_scrollbar_event = matches!(
            mouse_event.kind,
            MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left)
                | MouseEventKind::Up(MouseButton::Left)
        );
        if !is_scrollbar_event {
            return TranscriptScrollbarMouseHandling::NotHandled;
        }

        let Some(scrollbar_area) = scrollbar_area else {
            if matches!(mouse_event.kind, MouseEventKind::Up(MouseButton::Left)) {
                self.pointer_capture = false;
            }
            return if self.pointer_capture {
                TranscriptScrollbarMouseHandling::Handled
            } else {
                TranscriptScrollbarMouseHandling::NotHandled
            };
        };

        let is_over_scrollbar = mouse_event.column >= scrollbar_area.x
            && mouse_event.column < scrollbar_area.right()
            && mouse_event.row >= scrollbar_area.y
            && mouse_event.row < scrollbar_area.bottom();

        if !self.pointer_capture && !is_over_scrollbar {
            return TranscriptScrollbarMouseHandling::NotHandled;
        }

        let viewport_lines = transcript_area.height as usize;
        let scrollbar_is_visible = if viewport_lines > 0 && !transcript_cells.is_empty() {
            transcript_view_cache.ensure_wrapped(transcript_cells, transcript_area.width);
            let total_lines = transcript_view_cache.lines().len();
            let max_visible = std::cmp::min(total_lines, viewport_lines);
            is_transcript_scrollbar_active(total_lines, max_visible, *transcript_view_top)
        } else {
            false
        };

        // When the transcript is pinned to bottom, we intentionally hide the scrollbar (but still
        // reserve its column). In that state, we avoid hit-testing the scrollbar track so the
        // reserved column doesn't become an invisible interactive region. Pointer capture remains
        // active for an in-progress drag so a gesture that reaches the bottom doesn't fall through
        // into transcript selection mid-drag.
        if !self.pointer_capture && !scrollbar_is_visible {
            return TranscriptScrollbarMouseHandling::NotHandled;
        }

        if matches!(mouse_event.kind, MouseEventKind::Down(MouseButton::Left)) && is_over_scrollbar
        {
            self.pointer_capture = true;
        }

        if viewport_lines > 0 && !transcript_cells.is_empty() {
            // `ensure_wrapped` was already called above when checking visibility.
            let total_lines = transcript_view_cache.lines().len();
            let max_visible = std::cmp::min(total_lines, viewport_lines);
            let max_offset = total_lines.saturating_sub(max_visible);

            if let Some(next_offset) = transcript_scrollbar_offset_for_mouse_event(
                scrollbar_area,
                total_lines,
                max_visible,
                *transcript_view_top,
                mouse_event,
                &mut self.interaction,
                self.pointer_capture,
            ) {
                let next_offset = next_offset.min(max_offset);
                let line_meta = transcript_view_cache.line_meta();

                *transcript_scroll = if next_offset >= max_offset {
                    TranscriptScroll::ToBottom
                } else {
                    TranscriptScroll::anchor_for(line_meta, next_offset)
                        .unwrap_or(TranscriptScroll::ToBottom)
                };
                *transcript_view_top = next_offset.min(max_offset);
                *transcript_total_lines = total_lines;
                *mouse_scroll_state = MouseScrollState::default();
                tui.frame_requester().schedule_frame();
            }
        }

        if matches!(mouse_event.kind, MouseEventKind::Up(MouseButton::Left)) {
            self.pointer_capture = false;
        }

        TranscriptScrollbarMouseHandling::Handled
    }
}
