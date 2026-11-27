//! Manual working tree diffs use the invoking session's Git runner. Selecting the tab is
//! synchronous with the request, so a background completion only replaces its reader content.

use super::*;
use crate::backend_id::BackendId;
use crate::get_git_diff::get_git_diff;
use itertools::Itertools;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;

pub(super) struct DiffSource {
    thread: Option<ThreadId>,
    pub(super) pending: Option<(Uuid, AbortOnDropHandle<()>)>,
}

/// Keep the entire patch in order, but only decode ANSI and lay out nearby lines
/// as the reader scrolls. Continuations avoid blank rows between chunks.
#[derive(Debug)]
struct DiffChunk(String);

impl HistoryCell for DiffChunk {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.raw_lines()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        if self.0.is_empty() {
            vec!["No changes detected.".italic().into()]
        } else {
            self.0.lines().map(ansi_escape_line).collect()
        }
    }

    fn is_stream_continuation(&self) -> bool {
        true
    }
}

fn cells(text: &str) -> Vec<Arc<dyn HistoryCell>> {
    const LINES_PER_CHUNK: usize = 128;
    if text.trim().is_empty() {
        return vec![Arc::new(DiffChunk(String::new()))];
    }
    text.split_inclusive('\n')
        .chunks(LINES_PER_CHUNK)
        .into_iter()
        .map(|chunk| Arc::new(DiffChunk(chunk.collect())) as Arc<dyn HistoryCell>)
        .collect()
}

impl App {
    pub(in crate::app) async fn open_diff(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        thread_id: Option<ThreadId>,
        cwd: PathUri,
    ) -> Result<()> {
        let runner = self.workspace_command_runner.clone();
        let profile = self.chat_widget.diff_permission_profile();
        let path = cwd.to_path_buf();
        let load = async move {
            let Some(runner) = runner else {
                return "Failed to compute diff: workspace command runner unavailable".into();
            };
            match get_git_diff(runner.as_ref(), &path, profile.as_deref()).await {
                Ok((true, text)) => text,
                Ok((false, _)) => "`/diff` — _not inside a git repository_".into(),
                Err(error) => format!("Failed to compute diff: {error}"),
            }
        };
        let tx = self.app_event_tx.clone();
        if !self.side_retention_enabled(tui) || !self.sidebar_chooser_available() {
            tokio::spawn(async move {
                tx.send(AppEvent::DiffResult {
                    thread_id,
                    cwd,
                    text: load.await,
                });
            });
            return Ok(());
        }

        let backend = server.backend_id();
        let request = Uuid::new_v4();
        let task = tokio::spawn(async move {
            tx.send(AppEvent::SidebarDiffLoaded {
                request,
                text: load.await,
            });
        });
        let source = DocumentSource::Diff(DiffSource {
            thread: thread_id,
            pending: Some((request, AbortOnDropHandle::new(task))),
        });
        let parent = self.sidebar_parent();
        let pane = self.sidebar_mut();
        let tab = pane.diff_tab().unwrap_or(request);
        if let Some(doc) = pane.documents.get_mut(&tab)
            && doc.backend == backend
            && doc.path == cwd
            && matches!(&doc.source, DocumentSource::Diff(s) if s.thread.or(parent) == thread_id)
        {
            doc.source = source;
        } else {
            pane.documents.insert(
                tab,
                Document::new(
                    source,
                    cwd,
                    backend,
                    /*location*/ None,
                    State::Loading(Instant::now()),
                    DocumentAccess::User,
                ),
            );
        }
        pane.add_document_tab(tab);
        self.activate_sidebar_tab(tui, server, SidebarTab::Document(tab))
            .await
    }

    pub(in crate::app) async fn finish_sidebar_diff(
        &mut self,
        tui: &mut tui::Tui,
        backend: BackendId,
        request: Uuid,
        text: String,
    ) {
        // Looking up the request in its pane also covers the startup pane moving to the
        // first thread while Git runs.
        let Some((parent, tab, thread, owner, cwd)) =
            self.sidebars.iter().find_map(|(&parent, pane)| {
                pane.documents.iter().find_map(|(&tab, doc)| {
                    let DocumentSource::Diff(source) = &doc.source else {
                        return None;
                    };
                    (source.pending.as_ref()?.0 == request).then_some((
                        parent,
                        tab,
                        source.thread.or(parent),
                        doc.backend,
                        &doc.path,
                    ))
                })
            })
        else {
            return;
        };
        let current = if thread == self.chat_widget.thread_id() {
            owner == backend && cwd == self.chat_widget.session_cwd()
        } else if let Some(id) = thread {
            self.thread_backend_id(id).unwrap_or(backend) == owner
                && self.thread_cwd(id).await.as_ref() == Some(cwd)
        } else {
            false
        };
        let Some(pane) = self.sidebars.get_mut(&parent) else {
            return;
        };
        if !current {
            pane.remove_document(tab);
        } else if let Some(doc) = pane.documents.get_mut(&tab) {
            if let DocumentSource::Diff(source) = &mut doc.source {
                source.pending = None;
            }
            doc.state = doc.ready(cells(&text), Vec::new(), /*target*/ None);
        }
        tui.frame_requester().schedule_frame();
    }
}
