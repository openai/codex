//! Read-only Markdown previews use the focused thread's workspace and app-server.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::*;
use crate::motion::MotionMode;
use crate::sidebar_tab_strip::SidebarTab;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::visible_lines;
use crate::transcript_view::TranscriptView;
use codex_app_server_client::AppServerPath;
use codex_utils_path_uri::PathUri;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::widgets::Widget;

#[derive(Debug)]
struct FileCell {
    source: String,
}

impl HistoryCell for FileCell {
    fn raw_lines(&self) -> Vec<Line<'static>> {
        crate::history_cell::raw_lines_from_source(&self.source)
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        crate::markdown_render::render_markdown_lines_with_width_cwd_and_hidden_link_destinations(
            &self.source,
            Some(width.into()),
            /*cwd*/ None,
            &crate::markdown_render::hide_web_link_destination,
        )
    }
}

pub(super) struct Document {
    pub(super) path: PathUri,
    pub(super) backend: crate::backend_id::BackendId,
    state: State,
}

enum State {
    Loading(Instant),
    Failed(String),
    Ready {
        view: Box<TranscriptView>,
        cells: [Arc<dyn HistoryCell>; 1],
    },
}

impl Document {
    pub(super) fn connection_replaced(&mut self) {
        if matches!(self.state, State::Loading(_)) {
            self.state = State::Failed("Connection changed. Reopen this file to try again.".into());
        }
    }

    pub(super) fn loaded(&mut self, result: Result<String, String>) {
        self.state = match result {
            Ok(source) => {
                let cells: [Arc<dyn HistoryCell>; 1] = [Arc::new(FileCell { source })];
                let mut view = Box::<TranscriptView>::default();
                view.jump_to_beginning(&cells);
                State::Ready { view, cells }
            }
            Err(error) => State::Failed(error),
        };
    }

    pub(super) fn paint(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        motion: MotionMode,
        frames: &tui::FrameRequester,
        resize_hint: Option<&str>,
    ) {
        ratatui::widgets::Clear.render(area, buf);
        let mut body = area.inner(ratatui::layout::Margin::new(1, 0));
        if let Some(hint) = resize_hint
            && body.height > 1
        {
            body.height -= 1;
            crate::sidebar_chooser::resize_hint(hint, body.width).render(
                Rect {
                    y: body.bottom(),
                    height: 1,
                    ..body
                },
                buf,
            );
        }
        match &mut self.state {
            State::Loading(started) => {
                let glyph = crate::motion::loading_glyph(*started, motion, frames);
                Line::from(vec![
                    ratatui::text::Span::styled(glyph, crate::style::accent_style()),
                    " ".into(),
                    "Opening…".dim(),
                ])
                .render(body, buf);
            }
            State::Failed(error) => {
                Paragraph::new(format!("Could not open file: {error}"))
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .render(body, buf);
            }
            State::Ready { view, cells } => view.render(body, buf, cells),
        }
    }
}

impl App {
    pub(super) fn document_preview_available(
        &self,
        tui: &tui::Tui,
        server: &AppServerSession,
    ) -> bool {
        self.conversation_focus_available(tui)
            && self.sidebar_chooser_available()
            && server.thread_params_mode() != crate::app_server_session::ThreadParamsMode::Cloud
            && self.current_displayed_thread_id().is_some()
    }

    pub(super) async fn open_document(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        source: ThreadId,
        path: AppServerPath,
    ) -> Result<()> {
        let Some(parent) = self.sidebar_parent() else {
            return Ok(());
        };
        let resolver = server.request_view();
        let backend_id = resolver.backend_id();
        let tx = self.app_event_tx.clone();
        let request = uuid::Uuid::new_v4();
        self.sidebar_mut().focus_document_request = Some(request);
        tokio::spawn(async move {
            let resolved = resolver
                .resolve_thread_file(source, &path)
                .await
                .map_err(|error| format!("{error:#}"));
            tx.send(AppEvent::SidebarDocumentResolved {
                backend_id,
                parent,
                source,
                request,
                original_path: path,
                path: resolved,
            });
        });
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    pub(super) async fn open_resolved_document(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        source: ThreadId,
        original_path: AppServerPath,
        path: PathUri,
        focus: bool,
    ) -> Result<()> {
        let Some(parent) = self.sidebar_parent() else {
            return Ok(());
        };
        let backend_id = server.backend_id();
        let existing = self.sidebar().and_then(|pane| {
            pane.documents.iter().find_map(|(&id, doc)| {
                (doc.backend == backend_id && doc.path == path)
                    .then_some((id, matches!(doc.state, State::Failed(_))))
            })
        });
        if let Some((id, failed)) = existing {
            if !focus {
                return Ok(());
            }
            if !failed {
                return self
                    .activate_sidebar_tab(tui, server, SidebarTab::Document(id))
                    .await;
            }
            self.sidebar_mut().remove_document(id);
        }
        let request = uuid::Uuid::new_v4();
        let pane = self.sidebar_mut();
        pane.documents.insert(
            request,
            Document {
                path: path.clone(),
                backend: backend_id,
                state: State::Loading(Instant::now()),
            },
        );
        pane.add_document_tab(request);
        if focus {
            self.activate_sidebar_tab(tui, server, SidebarTab::Document(request))
                .await?;
        } else {
            tui.frame_requester().schedule_frame();
        }
        let reader = server.request_view();
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = reader
                .read_document(source, &original_path, &path)
                .await
                .map_err(|error| format!("{error:#}"));
            tx.send(AppEvent::SidebarDocumentLoaded {
                backend_id,
                parent,
                request,
                result,
            });
        });
        Ok(())
    }

    pub(super) fn handle_document_input(&mut self, tui: &mut tui::Tui, event: &TuiEvent) -> bool {
        let focused = self.panel_focus() == Some(PaneFocus::Sidebar);
        let area = self.conversation_frame.panel_area();
        let Some(pane) = self.sidebars.get_mut(&self.sidebar_parent()) else {
            return false;
        };
        let Some(SidebarContent::Document(id)) = pane.visible_content() else {
            return false;
        };
        match event {
            TuiEvent::Key(key) if focused => {
                if key.kind != KeyEventKind::Release
                    && let Some(Document {
                        state: State::Ready { view, cells },
                        ..
                    }) = pane.documents.get_mut(&id)
                {
                    view.navigate_pager(*key, cells, &self.keymap.pager);
                }
            }
            TuiEvent::Paste(_) if focused => {}
            TuiEvent::Mouse(mouse) if mouse.kind != MouseEventKind::Moved => {
                let inside =
                    area.is_some_and(|area| area.contains(Position::new(mouse.column, mouse.row)));
                if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                    pane.show(
                        SidebarContent::Document(id),
                        if inside {
                            PaneFocus::Sidebar
                        } else {
                            PaneFocus::Main
                        },
                    );
                }
                if !inside {
                    return false;
                }
                if let Some(Document {
                    state: State::Ready { view, cells },
                    ..
                }) = pane.documents.get_mut(&id)
                    && matches!(
                        mouse.kind,
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    )
                {
                    view.mouse_scroll_speed = self.transcript_view.mouse_scroll_speed;
                    let _ = view.handle_mouse(*mouse, cells);
                }
            }
            _ => return false,
        }
        tui.frame_requester().schedule_frame();
        true
    }
}

#[cfg(test)]
#[path = "sidebar_document_tests.rs"]
mod tests;
