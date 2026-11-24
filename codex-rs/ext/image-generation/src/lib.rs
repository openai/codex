mod artifact;
mod backend;
mod extension;
mod tool;

pub use extension::install;

pub(crate) const IMAGE_GEN_NAMESPACE: &str =
    codex_config::types::ImageGenerationSettings::NAMESPACE;
pub(crate) const IMAGEGEN_TOOL_NAME: &str = "imagegen";
