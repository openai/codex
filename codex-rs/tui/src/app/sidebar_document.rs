//! Read-only Markdown previews use the focused thread's workspace and app-server.

pub(super) mod model_read;
mod open_error;
mod watch;

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::*;
use crate::custom_terminal::Frame;
use crate::key_hint::KeyBinding;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use crate::markdown_render::FileLink;
use crate::markdown_render::SourceLocation;
use crate::motion::MotionMode;
use crate::pets::ImageProtocol;
use crate::sidebar_image::DocumentContent;
use crate::sidebar_image::SidebarImage;
use crate::sidebar_tab_strip::SidebarTab;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::ReaderLink;
use crate::terminal_hyperlinks::visible_lines;
use crate::transcript_view::DocumentPosition;
use crate::transcript_view::TranscriptView;
use crate::transcript_view::ViewAction;
use codex_app_server_client::AppServerPath;
use codex_app_server_client::ThreadPermissions;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use color_eyre::eyre::eyre;
use ratatui::layout::Position;
use ratatui::widgets::Widget;
use tokio::sync::oneshot;
use unicode_segmentation::UnicodeSegmentation;

async fn decode_document(
    bytes: Vec<u8>,
    path: &PathUri,
    location: Option<SourceLocation>,
) -> Result<DocumentContent> {
    if !(FileLink {
        path: path.inferred_native_path_string(),
        location,
    })
    .is_image()
    {
        return AppServerSession::preview_text(bytes).map(DocumentContent::Text);
    }
    tokio::task::spawn_blocking(move || match SidebarImage::decode(&bytes) {
        Ok(image) => Ok(DocumentContent::Image(Box::new(image))),
        Err(image_error) => AppServerSession::preview_text(bytes)
            .map(DocumentContent::Text)
            .map_err(|_| image_error),
    })
    .await?
}

#[derive(Debug)]
struct FileCell {
    source: String,
    language: Option<String>,
}

impl HistoryCell for FileCell {
    fn copy_source(&self) -> Option<&str> {
        Some(&self.source)
    }

    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.display_hyperlink_lines(width)
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        if let Some(language) = &self.language {
            return crate::render::highlight::highlight_code_to_lines(&self.source, language)
                .into_iter()
                .map(HyperlinkLine::new)
                .collect();
        }
        crate::markdown_render::render_markdown_lines_with_width_cwd_and_hidden_link_destinations(
            &self.source,
            Some(width.into()),
            /*cwd*/ None,
            &crate::markdown_render::hide_reader_link_destination,
        )
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        crate::history_cell::raw_lines_from_source(&self.source)
    }
}

pub(super) struct DocumentRenderOptions<'a> {
    pub(super) motion: MotionMode,
    pub(super) frames: &'a tui::FrameRequester,
    pub(super) keymap: &'a RuntimeKeymap,
    pub(super) resize_hint: Option<&'a str>,
    pub(super) image_protocol: Option<ImageProtocol>,
}

const MAX_MODEL_DOCUMENTS: usize = 8;

pub(super) enum DocumentOrigin {
    User {
        focus: bool,
    },
    Model {
        permissions: Result<Option<ThreadPermissions>>,
    },
}

enum DocumentAccess {
    User,
    Model { load: Option<ModelLoad> },
}

struct ModelLoad {
    permissions: Arc<ThreadPermissions>,
    // Dropping the tab or replacing the backend cancels the read.
    _cancel: oneshot::Sender<()>,
}

pub(super) struct Document {
    source: ThreadId,
    pub(super) path: PathUri,
    original_path: AppServerPath,
    pub(super) watch: Option<(uuid::Uuid, tokio::sync::watch::Sender<()>)>,
    pub(super) backend: crate::backend_id::BackendId,
    link: Option<usize>,
    location: Option<SourceLocation>,
    state: State,
    access: DocumentAccess,
}

enum State {
    AwaitingOpen,
    Loading(Instant),
    Failed {
        error: color_eyre::Report,
        position: Option<DocumentPosition>,
    },
    Image(Box<SidebarImage>),
    Ready {
        view: Box<TranscriptView>,
        cells: [Arc<dyn HistoryCell>; 1],
        line_starts: Vec<usize>,
        target: Option<(usize, usize)>,
    },
}

impl Document {
    pub(super) fn link_at(&self, column: u16, row: u16) -> Option<ReaderLink> {
        match &self.state {
            State::Ready { view, .. } if !view.is_select_mode() => view.reader_link_at(column, row),
            _ => None,
        }
    }

    pub(super) fn owns_interaction_key(&self, key: KeyEvent) -> bool {
        matches!(&self.state, State::Ready { view, .. }
            if view.has_active_interaction() && view.owns_interaction_key(key))
    }

    pub(super) fn allows_tab_shortcut(&self, key: KeyEvent) -> bool {
        match &self.state {
            State::Ready { view, target, .. } => {
                !view.has_active_interaction()
                    && (key.code != KeyCode::Esc || (self.link.is_none() && target.is_none()))
            }
            _ => true,
        }
    }

    pub(super) fn is_select_mode(&self) -> bool {
        matches!(&self.state, State::Ready { view, .. } if view.is_select_mode())
    }

    pub(super) fn is_search_editing(&self) -> bool {
        matches!(&self.state, State::Ready { view, .. } if view.is_search_editing())
    }

    pub(super) fn reader(&mut self) -> Option<(&mut TranscriptView, &[Arc<dyn HistoryCell>])> {
        match &mut self.state {
            State::Ready { view, cells, .. } => Some((view, cells)),
            _ => None,
        }
    }

    fn title(&self) -> String {
        self.path.basename().unwrap_or_default()
    }

    pub(super) fn awaits_open(&self) -> bool {
        matches!(self.state, State::AwaitingOpen)
    }

    fn is_model_snapshot(&self) -> bool {
        matches!(self.access, DocumentAccess::Model { .. })
            && matches!(self.state, State::Ready { .. } | State::Image(_))
    }

    pub(super) fn model_loading(&self) -> bool {
        matches!(self.access, DocumentAccess::Model { load: Some(_) })
    }

    pub(super) async fn finish_model_load(
        &mut self,
        server: &AppServerSession,
        result: Result<DocumentContent>,
    ) {
        let DocumentAccess::Model {
            load: Some(initial),
        } = &mut self.access
        else {
            return;
        };
        let initial = Arc::clone(&initial.permissions);
        self.access = DocumentAccess::Model { load: None };
        let result = match result {
            Ok(content) => match server.model_preview_permissions(self.source).await {
                Ok(Some(current))
                    if initial.profile == current.profile
                        && initial.workspace_roots == current.workspace_roots =>
                {
                    Ok(content)
                }
                _ => Err(eyre!(
                    "Workspace or permissions changed while loading the preview"
                )),
            },
            Err(error) => Err(error),
        };
        self.loaded(result);
    }

    pub(super) fn connection_replaced(&mut self) {
        self.watch = None;
        if let DocumentAccess::Model { load } = &mut self.access {
            *load = None;
        }
        if matches!(self.state, State::Loading(_)) {
            self.state = State::Failed {
                error: eyre!("Connection changed. Reopen this file to try again."),
                position: None,
            };
        }
    }

    pub(super) fn loaded(&mut self, result: Result<DocumentContent>) {
        let position = match &self.state {
            State::Ready { view, .. } => Some(view.document_position()),
            State::Failed { position, .. } => *position,
            State::AwaitingOpen | State::Loading(_) | State::Image(_) => None,
        };
        self.state = match result {
            Ok(DocumentContent::Image(image)) => State::Image(image),
            Ok(DocumentContent::Text(source)) => {
                if let State::Ready { cells, .. } = &self.state
                    && cells[0].copy_source() == Some(source.as_str())
                {
                    return;
                }
                self.link = None;
                let title = self.title();
                let code =
                    self.location.is_some() || !crate::markdown_render::is_markdown_file(&title);
                const MAX_SOURCE_BYTES: usize = 1024 * 1024;
                if code && source.len() > MAX_SOURCE_BYTES {
                    self.state = State::Failed {
                        error: eyre!("Source preview limit is 1 MiB"),
                        position,
                    };
                    return;
                }
                // Like the transcript layout, count one LF per logical line, including CRLF input.
                let mut start = 0;
                let line_starts = if code {
                    source
                        .lines()
                        .map(|line| {
                            let offset = start;
                            start += line.len() + 1;
                            offset
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                const MAX_SOURCE_LINES: usize = 10_000;
                if line_starts.len() > MAX_SOURCE_LINES {
                    self.state = State::Failed {
                        error: eyre!("Source preview limit is {MAX_SOURCE_LINES} lines"),
                        position,
                    };
                    return;
                }
                let language = code.then(|| {
                    title
                        .rsplit_once('.')
                        .map_or(title.as_str(), |(_, ext)| ext)
                        .to_owned()
                });
                let target = self
                    .location
                    .and_then(|loc| source_target(&source, &line_starts, loc));
                let cells: [Arc<dyn HistoryCell>; 1] = [Arc::new(FileCell { source, language })];
                let mut view = Box::<TranscriptView>::default();
                view.document_links = true;
                view.show_follow_control = false;
                match position {
                    Some(DocumentPosition::Latest) => view.jump_to_latest(),
                    Some(DocumentPosition::Offset(offset)) => view.jump_to_offset(&cells, offset),
                    None => view.jump_to_offset(&cells, target.map_or(0, |(_, offset)| offset)),
                }
                State::Ready {
                    view,
                    cells,
                    line_starts,
                    target,
                }
            }
            Err(error) => State::Failed { error, position },
        };
    }

    fn go_to(&mut self, location: Option<SourceLocation>) {
        let was_located = self.location.is_some();
        let restore_markdown = location.is_none()
            && was_located
            && crate::markdown_render::is_markdown_file(&self.title());
        self.location = location;
        self.link = None;
        let State::Ready {
            view,
            cells,
            line_starts,
            target,
        } = &mut self.state
        else {
            return;
        };
        let Some(source) = cells[0].copy_source() else {
            return;
        };
        if restore_markdown || (location.is_some() && line_starts.is_empty()) {
            // Coordinate links show source. Plain Markdown links restore rendering.
            let source = source.to_owned();
            self.state = State::Loading(Instant::now());
            self.loaded(Ok(DocumentContent::Text(source)));
        } else {
            *target = location.and_then(|location| source_target(source, line_starts, location));
            if let Some((_, offset)) = target {
                view.cancel_search();
                view.end_selection(cells);
                view.jump_to_offset(cells, *offset);
            }
        }
    }

    pub(super) fn paint(
        &mut self,
        area: Rect,
        frame: &mut Frame<'_>,
        options: DocumentRenderOptions<'_>,
    ) -> Option<Position> {
        let DocumentRenderOptions {
            motion,
            frames,
            keymap,
            resize_hint,
            image_protocol,
        } = options;
        let buf = &mut *frame.buffer;
        ratatui::widgets::Clear.render(area, buf);
        let mut body = area.inner(ratatui::layout::Margin::new(1, 0));
        if self.is_model_snapshot() && body.height > 1 {
            "Snapshot · Enter to open live"
                .dim()
                .render(Rect { height: 1, ..body }, buf);
            body.y += 1;
            body.height -= 1;
        }
        match &mut self.state {
            State::AwaitingOpen => {
                let path = self.path.inferred_native_path_string();
                let path =
                    crate::history_cell::sanitize_user_text(path.into()).replace(['\n', '\t'], " ");
                let title = match self.location {
                    Some(location) => format!("Open {path} at line {}?", location.line),
                    None => format!("Open {path}?"),
                };
                let mut lines = textwrap::wrap(&title, usize::from(body.width).max(1))
                    .into_iter()
                    .map(|line| Line::from(line.into_owned()))
                    .collect::<Vec<_>>();
                lines.push("Focus this pane and press Enter to open.".dim().into());
                Paragraph::new(lines)
                    .wrap(ratatui::widgets::Wrap { trim: true })
                    .render(body, buf);
            }
            State::Image(image) => {
                frame
                    .image_placements
                    .extend(image.paint(body, buf, image_protocol, resize_hint))
            }
            State::Loading(started) => {
                let glyph = crate::motion::loading_glyph(*started, motion, frames);
                Line::from(vec![
                    ratatui::text::Span::styled(glyph, crate::style::accent_style()),
                    " ".into(),
                    "Opening…".dim(),
                ])
                .render(body, buf);
            }
            State::Failed { error, .. } => {
                let mut lines = open_error::lines(&self.path, error, body.width);
                // On short panes, keep the space for the filename and recovery message.
                if lines.len() + 2 < usize::from(body.height) {
                    lines.insert(0, Line::default());
                    if resize_hint.is_none() {
                        let esc = crate::key_hint::plain(KeyCode::Esc);
                        let footer = Rect::new(body.x, body.bottom() - 1, body.width, 1);
                        crate::footer_hint::shortcut(&esc.display_label(), "close")
                            .render(footer, buf);
                    }
                }
                Paragraph::new(lines).render(body, buf);
            }
            State::Ready {
                view,
                cells,
                line_starts,
                target,
            } => {
                view.set_keymap_bindings(keymap);
                if view.tick_selection(cells) {
                    frames.schedule_frame_in(std::time::Duration::from_millis(45));
                }
                if view.advance_search(cells) {
                    frames.schedule_frame();
                }
                let footer = view.footer_with_navigation(body.width, motion, "Esc close");
                let hints = crate::footer_hint::HintLine::new(
                    [
                        Some(crate::footer_hint::shortcut("↑/↓", "read")),
                        Some(crate::footer_hint::shortcut(
                            &keymap
                                .pager
                                .find
                                .first()
                                .map_or_else(|| "F3".to_owned(), KeyBinding::display_label),
                            "find",
                        )),
                        Some(crate::footer_hint::shortcut("Tab", "link")),
                        keymap
                            .primary_hint(KeymapContext::Global, "select_transcript")
                            .map(|hint| {
                                crate::footer_hint::shortcut(&hint.display_label(), "select")
                            }),
                        Some(crate::footer_hint::shortcut(
                            "Esc",
                            if target.is_some() {
                                "clear highlight"
                            } else {
                                "close"
                            },
                        )),
                    ]
                    .into_iter()
                    .flatten(),
                    body.width,
                );
                let text: ratatui::text::Text<'_> = if let Some(hint) = resize_hint {
                    crate::sidebar_chooser::resize_hint(hint, body.width).into()
                } else if let Some(footer) = &footer
                    && view.has_active_interaction()
                {
                    footer.text.clone()
                } else {
                    hints.line.into()
                };
                let now = Instant::now();
                let height = (text.height() as u16).min(body.height.saturating_sub(1));
                let gap_height = u16::from(height > 0 && body.height - height > 1);
                let content = Rect {
                    height: body.height - height - gap_height,
                    ..body
                };
                let mut viewport = content;
                if content.width > 2 {
                    viewport.width -= 1;
                }
                view.render_document(
                    viewport,
                    buf,
                    cells,
                    line_starts,
                    target.filter(|_| !view.has_active_interaction()),
                );
                if let Some(index) = self.link {
                    view.paint_document_link(cells, index, buf);
                }
                let gap =
                    (gap_height > 0).then(|| Rect::new(body.x, content.bottom(), body.width, 1));
                view.paint_document_scroll_indicators(content, buf, cells);
                if let Some(delay) = view.render_composer_gap(gap, /*hint*/ None, buf, now) {
                    frames.schedule_frame_in(delay);
                }
                if height > 0 {
                    let footer_area =
                        Rect::new(body.x, content.bottom() + gap_height, body.width, height);
                    Paragraph::new(text).render(footer_area, buf);
                    if resize_hint.is_none()
                        && let Some(column) = footer.and_then(|footer| footer.cursor_column)
                    {
                        return Some(Position::new(body.x + column, footer_area.y));
                    }
                }
            }
        }
        if !matches!(self.state, State::Ready { .. })
            && body.height > 0
            && let Some(hint) = resize_hint
        {
            crate::sidebar_chooser::resize_hint(hint, body.width)
                .render(Rect::new(body.x, body.bottom() - 1, body.width, 1), buf);
        }
        None
    }
}

fn source_target(
    source: &str,
    starts: &[usize],
    location: SourceLocation,
) -> Option<(usize, usize)> {
    let index = (location.line - 1).min(starts.len().saturating_sub(1));
    let start = *starts.get(index)?;
    let line = source.lines().nth(index)?;
    let column = line
        .char_indices()
        .nth(location.column - 1)
        .map_or(line.len(), |(byte, _)| byte);
    let column = if column == line.len() {
        column
    } else {
        line.grapheme_indices(/*is_extended*/ true)
            .take_while(|(byte, _)| *byte <= column)
            .last()
            .map_or(0, |(byte, _)| byte)
    };
    Some((index + 1, start + column))
}

impl App {
    pub(super) fn cancel_hidden_sidebar_primary(&mut self, tui: &tui::Tui) {
        let parent = self.sidebar_parent();
        let visible = tui.is_owned_screen()
            && tui.is_terminal_focused()
            && self.overlay.is_none()
            && self.conversation_frame.panel_area().is_some();
        for (owner, pane) in &mut self.sidebars {
            pane.cancel_hidden_document_primary(visible && *owner == parent);
        }
    }

    pub(super) fn focused_document(&self) -> Option<&Document> {
        if self.panel_focus() != Some(PaneFocus::Sidebar) {
            return None;
        }
        let pane = self.sidebar()?;
        let SidebarContent::Document(id) = pane.visible_content()? else {
            return None;
        };
        pane.documents.get(&id)
    }

    pub(super) fn document_preview_available(
        &self,
        tui: &tui::Tui,
        server: &AppServerSession,
    ) -> bool {
        self.conversation_focus_available(tui)
            && self.sidebar_chooser_available()
            && server.supports_document_previews()
            && self.current_displayed_thread_id().is_some()
    }

    pub(super) fn refresh_transcript_document_links(&self) {
        crate::markdown_render::preferences::set_document_links(
            self.local_settings.workspace_enabled(&self.config)
                && self.app_server_target.thread_params_mode() != crate::ThreadParamsMode::Cloud,
        );
    }

    pub(super) async fn open_document(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        source: ThreadId,
        link: FileLink,
    ) -> Result<()> {
        let location = link.location;
        let path = AppServerPath::from_app_server(link.path);
        let Some(parent) = self.sidebar_parent() else {
            return Ok(());
        };
        let resolver = server.file_system();
        let backend_id = server.backend_id();
        let tx = self.app_event_tx.clone();
        let request = uuid::Uuid::new_v4();
        let pane = self.sidebar_mut();
        pane.agents.pending_link = None;
        pane.focus_document_request = Some(request);
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
                location,
            });
        });
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The safe read needs both paths, and the caller supplies navigation and focus."
    )]
    pub(super) async fn open_resolved_document(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        source: ThreadId,
        original_path: AppServerPath,
        path: PathUri,
        location: Option<SourceLocation>,
        focus: bool,
    ) -> Result<()> {
        if let Some(id) = self.open_document_tab(
            server,
            source,
            original_path,
            path,
            location,
            DocumentOrigin::User { focus },
        ) {
            if focus {
                self.activate_sidebar_tab(tui, server, SidebarTab::Document(id))
                    .await?;
            } else {
                tui.frame_requester().schedule_frame();
            }
        }
        Ok(())
    }

    pub(super) fn open_document_tab(
        &mut self,
        server: &AppServerSession,
        source: ThreadId,
        original_path: AppServerPath,
        path: PathUri,
        location: Option<SourceLocation>,
        origin: DocumentOrigin,
    ) -> Option<uuid::Uuid> {
        let parent = self.sidebar_parent()?;
        let backend_id = server.backend_id();
        let tx = self.app_event_tx.clone();
        let environment = self.environment_manager.try_local_environment();
        let existing = self.sidebar().and_then(|pane| {
            pane.documents
                .iter()
                .find(|(_, doc)| doc.backend == backend_id && doc.path == path)
                .map(|(&id, _)| id)
        });
        if let Some(id) = existing {
            let doc = self.sidebar_mut().documents.get_mut(&id)?;
            match (&origin, &doc.access) {
                (DocumentOrigin::User { focus: false }, _) => return None,
                (DocumentOrigin::User { focus: true }, DocumentAccess::User)
                    if !matches!(doc.state, State::Failed { .. }) =>
                {
                    doc.go_to(location);
                    return Some(id);
                }
                (DocumentOrigin::Model { permissions: Ok(_) }, DocumentAccess::User)
                    if doc.source == source =>
                {
                    // The user already authorized this tab. Keep the watch and use its content.
                    if location.is_some() {
                        doc.go_to(location);
                    }
                    return Some(id);
                }
                (DocumentOrigin::Model { .. }, DocumentAccess::User) => return None,
                _ => self.sidebar_mut().remove_document(id),
            }
        }
        if matches!(origin, DocumentOrigin::Model { .. })
            && self
                .sidebars
                .values()
                .flat_map(|pane| pane.documents.values())
                .filter(|doc| matches!(doc.access, DocumentAccess::Model { .. }))
                .count()
                >= MAX_MODEL_DOCUMENTS
        {
            self.chat_widget.add_warning_message(
                "Close a model-opened sidebar tab to open another model preview.".to_owned(),
            );
            return None;
        }
        let request = uuid::Uuid::new_v4();
        let (state, access) = match origin {
            DocumentOrigin::User { .. } => (State::Loading(Instant::now()), DocumentAccess::User),
            DocumentOrigin::Model { permissions } => {
                let (state, load) = match (permissions, environment) {
                    (Ok(None), _) => (State::AwaitingOpen, None),
                    (Err(error), _) => (
                        State::Failed {
                            error,
                            position: None,
                        },
                        None,
                    ),
                    (Ok(Some(_)), None) => (
                        State::Failed {
                            error: eyre!("Local filesystem is unavailable"),
                            position: None,
                        },
                        None,
                    ),
                    (Ok(Some(permissions)), Some(environment)) => {
                        let reader = server.file_system();
                        let task_tx = tx.clone();
                        let task_path = path.clone();
                        let task_original = original_path.clone();
                        let permissions = Arc::new(permissions);
                        let initial = Arc::clone(&permissions);
                        let (cancel, canceled) = oneshot::channel();
                        tokio::spawn(async move {
                            let read = model_read::read(
                                &reader,
                                &environment,
                                source,
                                &task_original,
                                &task_path,
                                &permissions,
                            );
                            let result = tokio::select! {
                                biased;
                                _ = canceled => Err(eyre!("Preview closed")),
                                result = read => result,
                            };
                            let result = match result {
                                Ok(bytes) => decode_document(bytes, &task_path, location).await,
                                Err(error) => Err(error),
                            };
                            task_tx.send(AppEvent::SidebarDocumentLoaded {
                                backend_id,
                                parent,
                                tab: request,
                                request,
                                result,
                            });
                        });
                        (
                            State::Loading(Instant::now()),
                            Some(ModelLoad {
                                permissions: initial,
                                _cancel: cancel,
                            }),
                        )
                    }
                };
                (state, DocumentAccess::Model { load })
            }
        };
        let pane = self.sidebar_mut();
        if matches!(access, DocumentAccess::User) && !pane.opened_files.contains(&path) {
            pane.opened_files.push(path.clone());
        }
        let mut doc = Document {
            source,
            path,
            original_path,
            watch: None,
            backend: backend_id,
            link: None,
            location,
            state,
            access,
        };
        doc.subscribe(request, parent, server, tx);
        pane.documents.insert(request, doc);
        pane.add_document_tab(request);
        Some(request)
    }

    pub(super) async fn handle_document_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> bool {
        let focused = self.panel_focus() == Some(PaneFocus::Sidebar);
        let area = self.conversation_frame.panel_area();
        let parent = self.sidebar_parent();
        let Some(pane) = self.sidebars.get_mut(&parent) else {
            return false;
        };
        let Some(SidebarContent::Document(id)) = pane.visible_content() else {
            return false;
        };
        if let TuiEvent::Mouse(mouse) = event
            && mouse.kind == MouseEventKind::Moved
        {
            if let Some(Document {
                state: State::Ready { view, cells, .. },
                ..
            }) = pane.documents.get_mut(&id)
                && view.handle_mouse(*mouse, cells).is_some()
            {
                tui.frame_requester().schedule_frame();
            }
            return false;
        }
        let action = match event {
            TuiEvent::Key(key) if focused => {
                if key.kind == KeyEventKind::Release {
                    return true;
                }
                let Some(doc) = pane.documents.get_mut(&id) else {
                    return true;
                };
                if doc.backend != server.backend_id() {
                    return true;
                }
                let escape = crate::key_hint::plain(KeyCode::Esc).is_press(*key);
                if key.kind != KeyEventKind::Press && (escape || key.code == KeyCode::Enter) {
                    return true;
                }
                if crate::key_hint::plain(KeyCode::Enter).is_press(*key)
                    && let Some(parent) = parent
                    && (doc.awaits_open()
                        || (doc.is_model_snapshot()
                            && doc.link.is_none()
                            && !doc.owns_interaction_key(*key)))
                {
                    doc.access = DocumentAccess::User;
                    if doc.awaits_open() {
                        doc.state = State::Loading(Instant::now());
                    }
                    doc.subscribe(id, parent, server, self.app_event_tx.clone());
                    if !pane.opened_files.contains(&doc.path) {
                        pane.opened_files.push(doc.path.clone());
                    }
                    tui.frame_requester().schedule_frame();
                    return true;
                }
                if let State::Ready {
                    view,
                    cells,
                    target,
                    ..
                } = &mut doc.state
                {
                    if escape && view.has_active_interaction() && !view.is_select_mode() {
                        view.cancel_search();
                        view.end_selection(cells);
                        None
                    } else if escape
                        && (doc.link.take().is_some()
                            || (!view.has_active_interaction() && target.take().is_some()))
                    {
                        None
                    } else if !view.is_search_editing()
                        && (self.keymap.app.find_transcript.is_pressed(*key)
                            || self.keymap.pager.find.is_pressed(*key))
                    {
                        view.begin_search();
                        doc.link = None;
                        None
                    } else if !view.is_select_mode()
                        && self.keymap.app.select_transcript.is_pressed(*key)
                    {
                        view.begin_select_mode(cells);
                        doc.link = None;
                        None
                    } else if !view.has_active_interaction()
                        && view.navigate_pager(*key, cells, &self.keymap.pager)
                    {
                        doc.link = None;
                        Some(ViewAction::Changed)
                    } else if !view.has_active_interaction()
                        && let (code @ (KeyCode::Tab | KeyCode::BackTab), KeyModifiers::NONE) =
                            crate::key_hint::normalize_key_parts(key.code, key.modifiers)
                    {
                        let forward = code == KeyCode::Tab;
                        doc.link = view.cycle_document_link(cells, doc.link, forward);
                        None
                    } else if crate::key_hint::plain(KeyCode::Enter).is_press(*key)
                        && let Some(index) = doc.link.take()
                    {
                        view.document_link(cells, index).map(ViewAction::from)
                    } else {
                        doc.link = None;
                        if view.has_active_interaction() {
                            view.handle_key(*key, cells).or_else(|| {
                                view.navigate_pager(*key, cells, &self.keymap.pager)
                                    .then_some(ViewAction::Changed)
                            })
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            }
            TuiEvent::Paste(text) if focused => {
                if let Some(Document {
                    state: State::Ready { view, .. },
                    ..
                }) = pane.documents.get_mut(&id)
                {
                    view.paste_search(text);
                }
                None
            }
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
                let selection_motion = matches!(
                    mouse.kind,
                    MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
                );
                if (selection_motion && !focused) || (!selection_motion && !inside) {
                    return false;
                }
                if let Some(Document {
                    state: State::Ready { view, cells, .. },
                    link,
                    ..
                }) = pane.documents.get_mut(&id)
                {
                    *link = None;
                    view.mouse_scroll_speed = self.transcript_view.mouse_scroll_speed;
                    view.copy_on_select = self.transcript_view.copy_on_select;
                    view.copy_on_select_clear_selection =
                        self.transcript_view.copy_on_select_clear_selection;
                    view.primary_selection = self.transcript_view.primary_selection;
                    let mut mouse = *mouse;
                    if matches!(
                        mouse.kind,
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    ) {
                        mouse.column = mouse.column.max(view.remaining_area().x);
                    }
                    view.handle_mouse(mouse, cells)
                } else {
                    None
                }
            }
            _ => return false,
        };
        match action {
            Some(ViewAction::OpenDocument(link)) => {
                if let Some(doc) = pane.documents.get(&id)
                    && let Some(parent) = doc.path.parent()
                {
                    let result = match LegacyAppPathString::from_string(link.path)
                        .resolve_against(&parent, /*user_home_dir*/ None)
                    {
                        Ok(path) if !path.to_url().has_host() => {
                            let source = doc.source;
                            let original_path =
                                AppServerPath::from_app_server(path.inferred_native_path_string());
                            self.open_resolved_document(
                                tui,
                                server,
                                source,
                                original_path,
                                path,
                                link.location,
                                /*focus*/ true,
                            )
                            .await
                        }
                        Ok(_) => Ok(()),
                        Err(error) => Err(error.into()),
                    };
                    if let Err(error) = result {
                        self.chat_widget.add_error_message(error.to_string());
                    }
                }
            }
            Some(ViewAction::OpenLink(url)) => {
                if let Some(url) = crate::terminal_hyperlinks::web_destination(&url) {
                    self.open_url_in_browser(url);
                }
            }
            Some(action) => {
                if let Some((view, cells)) = pane.documents.get_mut(&id).and_then(Document::reader)
                    && let Some((count, result)) = view.copy_action(
                        tui,
                        cells,
                        &action,
                        crate::clipboard_copy::CopyFormat::Markdown,
                    )
                {
                    view.show_copy_feedback(&result, count);
                }
            }
            None => {}
        }
        tui.frame_requester().schedule_frame();
        true
    }
}

#[cfg(test)]
#[path = "sidebar_document_tests.rs"]
mod tests;
