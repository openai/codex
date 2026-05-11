use super::ContextualUserFragment;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ImageGenerationInstructions {
    image_output_dir: String,
    image_output_path: String,
    call_id: Option<String>,
    saved_path: Option<String>,
    revised_prompt: Option<String>,
}

impl ImageGenerationInstructions {
    pub(crate) fn for_generated_image(
        image_output_dir: impl Display,
        image_output_path: impl Display,
        call_id: impl Display,
        saved_path: impl Display,
        revised_prompt: Option<&str>,
    ) -> Self {
        Self {
            image_output_dir: image_output_dir.to_string(),
            image_output_path: image_output_path.to_string(),
            call_id: Some(call_id.to_string()),
            saved_path: Some(saved_path.to_string()),
            revised_prompt: revised_prompt.map(str::to_string),
        }
    }
}

impl ContextualUserFragment for ImageGenerationInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "";
    const END_MARKER: &'static str = "";

    fn body(&self) -> String {
        let mut body = format!(
            "Generated images are saved to {} as {} by default.\nIf you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.",
            self.image_output_dir, self.image_output_path
        );

        if let (Some(call_id), Some(saved_path)) = (&self.call_id, &self.saved_path) {
            body.push_str(&format!(
                "\n\nThe most recent image_generation_call completed and was saved locally.\nArtifact metadata:\n- call_id: {call_id}\n- saved_path: {saved_path}"
            ));
            if let Some(revised_prompt) = self.revised_prompt.as_deref()
                && !revised_prompt.is_empty()
            {
                body.push_str(&format!("\n- revised_prompt: {revised_prompt}"));
            }
            body.push_str(
                "\n\nContinue the workflow now. If caller, user, or skill instructions ask for final message text alongside the image, provide it. If the generated image alone fully satisfies the request and no follow-up text is needed, keep any final response minimal or empty.",
            );
        }

        body
    }
}
