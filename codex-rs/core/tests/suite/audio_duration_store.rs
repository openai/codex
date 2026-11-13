//! Supplies measured clips and unavailable metadata for Core audio budgeting scenarios.

use codex_attachment_store::AttachmentMetadata;
use codex_attachment_store::AttachmentStore;
use codex_attachment_store::AttachmentStoreError;
use codex_attachment_store::AttachmentStoreErrorKind;
use codex_attachment_store::AudioMetadata;
use codex_attachment_store::FormatSpecificMetadata;
use codex_attachment_store::ImageMetadata;
use codex_attachment_store::InlineAttachmentStore;
use codex_attachment_store::ResolveFuture;
use codex_attachment_store::ResolveRequest;
use codex_attachment_store::UploadFuture;
use codex_attachment_store::UploadRequest;
use codex_attachment_store::UploadResult;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub(super) struct TestAudioDurationStore {
    pub(super) slow_lookup_started: tokio::sync::Notify,
    pub(super) measure_short_clips: bool,
    pub(super) tool_uploads: Mutex<Vec<UploadRequest>>,
    pub(super) tool_file_expired: AtomicBool,
    pub(super) tool_duration_reads: AtomicUsize,
}

impl AttachmentStore for TestAudioDurationStore {
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_> {
        if request
            .tags
            .get("usage")
            .is_some_and(|usage| usage == "tool_audio")
        {
            self.tool_uploads
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request);
            return Box::pin(async {
                Ok(UploadResult::File {
                    file_id: "file_tool_audio".into(),
                })
            });
        }
        InlineAttachmentStore.upload(request)
    }

    fn is_resolvable<'a>(
        &'a self,
        file_id: &'a str,
    ) -> codex_attachment_store::IsResolvableFuture<'a> {
        Box::pin(async move {
            file_id != "file_tool_audio" || !self.tool_file_expired.load(Ordering::SeqCst)
        })
    }

    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a> {
        Box::pin(async move {
            assert!(request.download_url_ttl.is_none());
            let format_specific = match request.file_id {
                "file_tool_audio" => {
                    self.tool_duration_reads.fetch_add(1, Ordering::SeqCst);
                    Some(FormatSpecificMetadata::Audio(AudioMetadata {
                        duration_ms: Some(500),
                    }))
                }
                "file_audio_1" | "file_audio_7" if self.measure_short_clips => {
                    Some(FormatSpecificMetadata::Audio(AudioMetadata {
                        duration_ms: Some(30_000),
                    }))
                }
                "file_audio_2" => Some(FormatSpecificMetadata::Image(ImageMetadata::default())),
                "file_audio_3" => {
                    return Err(AttachmentStoreError::new(
                        AttachmentStoreErrorKind::Backend,
                        "metadata unavailable",
                    ));
                }
                "file_audio_4" => Some(FormatSpecificMetadata::Audio(AudioMetadata {
                    duration_ms: Some(0),
                })),
                "file_audio_5" => {
                    self.slow_lookup_started.notify_one();
                    return std::future::pending().await;
                }
                "file_audio_6" => Some(FormatSpecificMetadata::Audio(AudioMetadata::default())),
                _ => None,
            };
            Ok(AttachmentMetadata {
                format_specific,
                ..AttachmentMetadata::default()
            })
        })
    }
}
