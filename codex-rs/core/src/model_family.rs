use crate::tool_apply_patch::ApplyPatchToolType;

/// A model family is a group of models that share certain characteristics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelFamily {
    /// The full model slug used to derive this model family, e.g.
    /// "gpt-4.1-2025-04-14".
    pub slug: String,

    /// The model family name, e.g. "gpt-4.1". Note this should able to be used
    /// with [`crate::openai_model_info::get_model_info`].
    pub family: String,

    /// True if the model needs additional instructions on how to use the
    /// "virtual" `apply_patch` CLI.
    pub needs_special_apply_patch_instructions: bool,

    // Whether the `reasoning` field can be set when making a request to this
    // model family. Note it has `effort` and `summary` subfields (though
    // `summary` is optional).
    pub supports_reasoning_summaries: bool,

    // This should be set to true when the model expects a tool named
    // "local_shell" to be provided. Its contract must be understood natively by
    // the model such that its description can be omitted.
    // See https://platform.openai.com/docs/guides/tools-local-shell
    pub uses_local_shell_tool: bool,

    /// Present if the model performs better when `apply_patch` is provided as
    /// a tool call instead of just a bash command
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
}

macro_rules! model_family {
    (
        $slug:expr, $family:expr $(, $key:ident : $value:expr )* $(,)?
    ) => {{
        // defaults
        let mut mf = ModelFamily {
            slug: $slug.to_string(),
            family: $family.to_string(),
            needs_special_apply_patch_instructions: false,
            supports_reasoning_summaries: false,
            uses_local_shell_tool: false,
            apply_patch_tool_type: None,
        };
        // apply overrides
        $(
            mf.$key = $value;
        )*
        Some(mf)
    }};
}

macro_rules! simple_model_family {
    (
        $slug:expr, $family:expr
    ) => {{
        Some(ModelFamily {
            slug: $slug.to_string(),
            family: $family.to_string(),
            needs_special_apply_patch_instructions: false,
            supports_reasoning_summaries: false,
            uses_local_shell_tool: false,
            apply_patch_tool_type: None,
        })
    }};
}

/// Returns a `ModelFamily` for the given model slug, or `None` if the slug
/// does not match any known model family.
pub fn find_family_for_model(slug: &str) -> Option<ModelFamily> {
    if slug.starts_with("o3") {
        model_family!(
            slug, "o3",
            supports_reasoning_summaries: true,
        )
    } else if slug.starts_with("o4-mini") {
        model_family!(
            slug, "o4-mini",
            supports_reasoning_summaries: true,
        )
    } else if slug.starts_with("codex-mini-latest") {
        model_family!(
            slug, "codex-mini-latest",
            supports_reasoning_summaries: true,
            uses_local_shell_tool: true,
        )
    } else if slug.starts_with("codex-") {
        model_family!(
            slug, slug,
            supports_reasoning_summaries: true,
        )
    } else if slug.starts_with("gpt-4.1") {
        model_family!(
            slug, "gpt-4.1",
            needs_special_apply_patch_instructions: true,
        )
    } else if slug.starts_with("gpt-oss") {
        model_family!(slug, "gpt-oss", apply_patch_tool_type: Some(ApplyPatchToolType::Function))
    } else if slug.starts_with("gpt-4o") {
        simple_model_family!(slug, "gpt-4o")
    } else if slug.starts_with("gpt-3.5") {
        simple_model_family!(slug, "gpt-3.5")
    } else if slug.starts_with("gpt-5") {
        model_family!(
            slug, "gpt-5",
            supports_reasoning_summaries: true,
        )
    } else {
        None
    }
}
