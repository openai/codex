use std::path::PathBuf;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::rpc::internal_error;

pub(crate) fn default_codex_home() -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    default_codex_home_path()
        .and_then(|path| {
            AbsolutePathBuf::from_absolute_path_checked(path)
                .map_err(|err| format!("runtime codex home is not absolute: {err}"))
        })
        .map_err(internal_error)
}

pub(crate) fn default_codex_home_path() -> Result<PathBuf, String> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "failed to locate home directory".to_string())?;
    Ok(home.join(".codex"))
}
