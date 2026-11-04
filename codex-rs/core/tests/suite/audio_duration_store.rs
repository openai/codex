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

#[derive(Default)]
pub(super) struct TestAudioDurationStore {
    pub(super) slow_lookup_started: tokio::sync::Notify,
    pub(super) measure_short_clips: bool,
}

impl AttachmentStore for TestAudioDurationStore {
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_> {
        InlineAttachmentStore.upload(request)
    }

    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a> {
        Box::pin(async move {
            assert!(request.download_url_ttl.is_none());
            let format_specific = match request.file_id {
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
