use super::ContextualUserFragment;
use std::fmt::Display;

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
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn body(&self) -> String {
        format!(
            "Generated images are saved to {} as {} by default.\nIf you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.",
            self.image_output_dir, self.image_output_path
        )
    }
}
