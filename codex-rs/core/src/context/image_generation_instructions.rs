use super::ContextualUserFragment;
use std::fmt::Display;

/// Returns the model-facing hint for the host's generated-image artifact path.
pub fn image_generation_output_hint(
    image_output_dir: impl Display,
    image_output_path: impl Display,
) -> String {
    format!(
        "Generated images are saved to {image_output_dir} as {image_output_path} by default.\nIf you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it."
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
    fn role() -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        image_generation_output_hint(&self.image_output_dir, &self.image_output_path)
    }
}
