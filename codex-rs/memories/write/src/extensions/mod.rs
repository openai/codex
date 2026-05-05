mod ad_hoc;
mod prune;

use std::path::Path;

pub(crate) async fn seed_extension_instructions(memory_root: &Path) -> std::io::Result<()> {
    ad_hoc::seed_instructions(memory_root).await
}

pub use prune::prune_old_extension_resources;
