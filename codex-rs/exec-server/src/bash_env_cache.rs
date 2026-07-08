#[cfg(not(unix))]
use std::collections::HashMap;

use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;

#[cfg(unix)]
#[path = "bash_env_cache_unix.rs"]
mod imp;

#[cfg(unix)]
pub(crate) use imp::BashEnvCache;

#[cfg(not(unix))]
#[derive(Default)]
pub(crate) struct BashEnvCache(());

#[cfg(not(unix))]
impl BashEnvCache {
    pub(crate) async fn prepare_launch(
        &self,
        params: &ExecParams,
        environment: &HashMap<String, String>,
        _runtime_paths: Option<&ExecServerRuntimePaths>,
    ) -> (Vec<String>, HashMap<String, String>) {
        (params.argv.clone(), environment.clone())
    }
}
