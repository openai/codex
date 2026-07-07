use super::ContextualUserFragment;
use std::fmt::Display;

/// Maximum size of the extension's model-facing generated-image path hint.
const MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES: usize = 1024;

/// Returns the extension's model-facing hint, or omits it if the path makes it too large.
pub fn extension_image_generation_output_hint(
    image_output_dir: impl Display,
    image_output_path: impl Display,
) -> Option<String> {
    let hint = image_generation_hint(image_output_dir, image_output_path);
    (hint.len() <= MAX_IMAGE_GENERATION_OUTPUT_HINT_BYTES).then_some(hint)
}

fn image_generation_hint(
    image_output_dir: impl Display,
    image_output_path: impl Display,
) -> String {
    format!(
        "Generated images are saved to {image_output_dir} as {image_output_path} by default.\nAlready displayed; do not repeat it or an unchanged copy in final Markdown unless asked.\nFor another path, copy it and keep the original unless asked to delete it."
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImageGenerationInstructions {
    image_output_dir: String,
    image_output_path: String,
}

impl ImageGenerationInstructions {
    pub(crate) fn new(image_output_dir: impl Display, image_output_path: impl Display) -> Self {
        Self {
            image_output_dir: image_output_dir.to_string(),
            image_output_path: image_output_path.to_string(),
        }
    }
}

impl ContextualUserFragment for ImageGenerationInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        image_generation_hint(&self.image_output_dir, &self.image_output_path)
    }
}
