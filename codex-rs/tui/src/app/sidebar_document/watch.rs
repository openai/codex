//! One subscription per open tab. Dropping or replacing its sender releases the server watch.

use super::AppEvent;
use super::AppEventSender;
use super::AppServerPath;
use super::AppServerSession;
use super::Document;
use super::DocumentContent;
use super::FileLink;
use super::SidebarImage;
use super::ThreadId;
use tokio::sync::watch;

impl Document {
    pub(in crate::app) fn subscribe(
        &mut self,
        tab: uuid::Uuid,
        parent: ThreadId,
        server: &AppServerSession,
        tx: AppEventSender,
    ) {
        let reader = server.request_view();
        let backend_id = self.backend;
        let source = self.source;
        let path = self.path.clone();
        let original_path = self.original_path.clone();
        let is_image = (FileLink {
            path: path.inferred_native_path_string(),
            location: self.location,
        })
        .is_image();
        let request = uuid::Uuid::new_v4();
        let (refresh, mut changes) = watch::channel(());
        self.watch = Some((request, refresh));
        tokio::spawn(async move {
            let fs = reader.file_system();
            let load = || async {
                let result = async {
                    if is_image {
                        let bytes = reader
                            .read_preview_file(source, &original_path, &path)
                            .await?;
                        tokio::task::spawn_blocking(move || match SidebarImage::decode(&bytes) {
                            Ok(image) => Ok(DocumentContent::Image(Box::new(image))),
                            Err(image_error) => AppServerSession::preview_text(bytes)
                                .map(DocumentContent::Text)
                                .map_err(|_| image_error),
                        })
                        .await?
                    } else {
                        reader
                            .read_document(source, &original_path, &path)
                            .await
                            .map(DocumentContent::Text)
                    }
                }
                .await;
                tx.send(AppEvent::SidebarDocumentLoaded {
                    backend_id,
                    parent,
                    tab,
                    request,
                    result,
                });
            };

            // Watch the parent so replacing the file atomically doesn't lose our subscription.
            // Register before reading so edits between the initial read and registration can't be missed.
            let watched = if let Some(dir) = path.parent() {
                let dir = AppServerPath::from_app_server(dir.inferred_native_path_string());
                fs.watch_document(&dir, request).await.is_ok()
            } else {
                false
            };
            while changes.has_changed().is_ok() {
                load().await;
                tokio::select! {
                    result = changes.changed() => if result.is_err() { break; },
                    // Older servers can read a file without supporting fs/watch.
                    _ = tokio::time::sleep(std::time::Duration::from_secs(3)), if !watched => {}
                }
            }
            if watched {
                fs.unwatch_document(request).await;
            }
        });
    }
}
