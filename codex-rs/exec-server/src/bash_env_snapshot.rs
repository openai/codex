#[cfg(not(unix))]
use std::collections::HashMap;

use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;

#[cfg(unix)]
#[path = "bash_env_snapshot_unix.rs"]
mod imp;

#[cfg(unix)]
pub(crate) use imp::BashEnvSnapshotCache;

#[cfg(not(unix))]
#[derive(Default)]
pub(crate) struct BashEnvSnapshotCache(());

#[cfg(not(unix))]
impl BashEnvSnapshotCache {
    pub(crate) async fn prepare_launch(
        &self,
        params: &ExecParams,
        environment: &HashMap<String, String>,
        _runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> (Vec<String>, HashMap<String, String>) {
        (params.argv.clone(), environment.clone())
    }

    pub(crate) fn clear(&self) {}
}
