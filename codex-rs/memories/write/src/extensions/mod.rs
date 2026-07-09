mod ad_hoc;
mod persistent;
mod prune;

use std::path::Path;

pub(crate) async fn seed_extension_instructions(memory_root: &Path) -> std::io::Result<()> {
    ad_hoc::seed_instructions(memory_root).await
}

pub use persistent::PersistentMemoryExtensionResource;
pub use persistent::PersistentMemoryExtensionSyncOutcome;
pub use persistent::persistent_extension_needs_sync;
pub use persistent::sync_persistent_extension_resources;
pub use prune::prune_old_extension_resources;
