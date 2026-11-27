//! Load local Markdown images off the draw thread, only for visible replies.

use std::path::Path;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::oneshot;

use crate::history_cell::HistoryCell;
use crate::pets::ImageProtocol;
use crate::transcript_image::PreviewKind;
use crate::transcript_image::TranscriptImage;
use crate::tui::FrameRequester;

// One bounded decoder in flight per view. Keep misses too, so an unavailable
// preview never causes a redraw loop. A new reply can reuse the same filename.
const CACHE_SIZE: usize = 32;
type Pixels = Option<Arc<TranscriptImage>>;

#[derive(Clone)]
pub(super) struct Request {
    pub(super) reply: Weak<dyn HistoryCell>,
    pub(super) path: Arc<Path>,
    pub(super) columns: u16,
    pub(super) protocol: ImageProtocol,
    pub(super) cell_size: Option<(u32, u32)>,
}

impl PartialEq for Request {
    fn eq(&self, other: &Self) -> bool {
        self.reply.ptr_eq(&other.reply)
            && self.path == other.path
            && self.columns == other.columns
            && self.protocol == other.protocol
            && self.cell_size == other.cell_size
    }
}

#[derive(Default)]
pub(super) struct MarkdownImages {
    cache: Vec<(Request, Pixels)>,
    loading: Option<oneshot::Receiver<(Request, Pixels)>>,
}

impl MarkdownImages {
    pub(super) fn get(&self, request: &Request) -> Option<&Pixels> {
        let (_, image) = self.cache.iter().find(|(key, _)| key == request)?;
        Some(image)
    }

    /// Return the cell whose cached text layout can now include a preview.
    pub(super) fn poll(&mut self) -> Option<Weak<dyn HistoryCell>> {
        let result = self.loading.as_mut()?.try_recv();
        if matches!(result, Err(oneshot::error::TryRecvError::Empty)) {
            return None;
        }
        self.loading = None;
        let (request, pixels) = result.ok()?;
        let updated = pixels.as_ref().map(|_| request.reply.clone());
        self.cache.push((request, pixels));
        updated
    }

    pub(super) fn request<'a>(
        &mut self,
        visible: impl Iterator<Item = &'a Request>,
        frames: &FrameRequester,
        local_workspace: impl Future<Output = bool> + Send + 'static,
    ) {
        if self.loading.is_some() {
            return;
        }
        let visible: Vec<_> = visible.collect();
        let Some(&request) = visible.iter().find(|r| self.get(r).is_none()) else {
            return;
        };
        if self.cache.len() >= CACHE_SIZE {
            // Never evict another preview on screen just to request it again next frame.
            let offscreen = self
                .cache
                .iter()
                .position(|(key, _)| !visible.contains(&key));
            let Some(index) = offscreen else { return };
            self.cache.remove(index);
        }
        let request = request.clone();
        let (tx, rx) = oneshot::channel();
        self.loading = Some(rx);
        let frames = frames.clone();
        tokio::spawn(async move {
            if !local_workspace.await {
                // Retry on a later draw if the workspace or connection changes.
                tokio::time::sleep(Duration::from_secs(/*secs*/ 5)).await;
                return;
            }
            tokio::task::spawn_blocking(move || {
                if tx.is_closed() || request.reply.strong_count() == 0 {
                    drop(tx);
                    frames.schedule_frame();
                    return;
                }
                let image = TranscriptImage::load_for_cells(
                    &request.path,
                    request.columns,
                    request.protocol,
                    PreviewKind::Generated,
                    request.cell_size,
                );
                if tx.send((request, image.map(Arc::new))).is_ok() {
                    frames.schedule_frame();
                }
            });
        });
    }
}
