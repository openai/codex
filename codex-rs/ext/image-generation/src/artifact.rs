use std::fmt::Display;

use codex_utils_absolute_path::AbsolutePathBuf;

const GENERATED_IMAGE_ARTIFACTS_DIR: &str = "generated_images";
const MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES: usize = 1024;

/// Identify GIFs from their encoded header so transport and filenames preserve animation.
#[derive(Clone, Copy)]
pub(crate) enum GeneratedImageFormat {
    Png,
    Gif,
}

impl GeneratedImageFormat {
    pub(crate) fn from_base64(result: &str) -> Self {
        let result = result.trim();
        if result.starts_with("R0lGODdh") || result.starts_with("R0lGODlh") {
            Self::Gif
        } else {
            Self::Png
        }
    }

    pub(crate) fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Gif => "image/gif",
        }
    }
}

/// Returns the extension-owned artifact path for a generated image.
pub(crate) fn image_generation_artifact_path(
    save_root: &AbsolutePathBuf,
    session_id: &str,
    call_id: &str,
    format: GeneratedImageFormat,
) -> AbsolutePathBuf {
    let sanitize = |value: &str| {
        let mut sanitized: String = value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        if sanitized.is_empty() {
            sanitized = "generated_image".to_string();
        }
        sanitized
    };

    let extension = match format {
        GeneratedImageFormat::Png => "png",
        GeneratedImageFormat::Gif => "gif",
    };
    save_root
        .join(GENERATED_IMAGE_ARTIFACTS_DIR)
        .join(sanitize(session_id))
        .join(format!("{}.{extension}", sanitize(call_id)))
}

/// Returns the model-facing generated-image path hint, or omits it if it is too large.
pub(crate) fn image_generation_output_hint(
    image_output_dir: impl Display,
    image_output_path: impl Display,
) -> Option<String> {
    let hint = format!(
        "Generated images are saved to {image_output_dir} as {image_output_path} by default.\nIf you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.\nThe generated image is already displayed to the user. There is no need to render it in the final response as a Markdown image or file link."
    );
    (hint.len() <= MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES).then_some(hint)
}
