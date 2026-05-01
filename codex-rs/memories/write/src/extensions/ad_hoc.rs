use crate::memory_extensions_root;
use std::path::Path;

pub(super) const INSTRUCTIONS: &str =
    include_str!("../../templates/extensions/ad_hoc/instructions.md");

pub(super) async fn seed_instructions(memory_root: &Path) -> std::io::Result<()> {
    let extension_root = memory_extensions_root(memory_root).join("ad_hoc");
    let instructions_path = extension_root.join("instructions.md");

    if tokio::fs::try_exists(&instructions_path).await? {
        return Ok(());
    }

    tokio::fs::create_dir_all(&extension_root).await?;
    tokio::fs::write(instructions_path, INSTRUCTIONS).await
}

#[cfg(test)]
#[path = "ad_hoc_tests.rs"]
mod tests;
