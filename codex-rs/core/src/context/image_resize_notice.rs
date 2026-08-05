use super::ContextualUserFragment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageResizeNoticeSource {
    UserMessage,
    ToolOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResizedImage {
    pub(crate) image_number: usize,
    pub(crate) image_count: usize,
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) prepared_width: u32,
    pub(crate) prepared_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageResizeNotice {
    source: ImageResizeNoticeSource,
    resized_images: Vec<ResizedImage>,
}

impl ImageResizeNotice {
    pub(crate) fn new(source: ImageResizeNoticeSource, resized_images: Vec<ResizedImage>) -> Self {
        Self {
            source,
            resized_images,
        }
    }
}

impl ContextualUserFragment for ImageResizeNotice {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<image_resize_notice>", "</image_resize_notice>")
    }

    fn body(&self) -> String {
        let source = match self.source {
            ImageResizeNoticeSource::UserMessage => "user message",
            ImageResizeNoticeSource::ToolOutput => "tool output",
        };
        let notices = self
            .resized_images
            .iter()
            .map(|image| {
                format!(
                    "Image {} of {} in the preceding {source} was resized from {}x{} to {}x{} pixels.",
                    image.image_number,
                    image.image_count,
                    image.source_width,
                    image.source_height,
                    image.prepared_width,
                    image.prepared_height,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n{notices}\n")
    }
}
