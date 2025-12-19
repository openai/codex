use dirs::home_dir;
use std::path::PathBuf;

/// This was copied from codex-core but codex-core depends on this crate.
/// TODO: move this to a shared crate lower in the dependency tree.
///
///
/// Returns the path to the Codexel configuration directory.
///
/// The directory can be specified by the `CODEXEL_HOME` environment variable.
/// For compatibility with existing installs, `CODEX_HOME` is also honored. When
/// neither is set, defaults to `~/.codexel`.
///
/// - If `CODEXEL_HOME` (or `CODEX_HOME`) is set, the value will be canonicalized and this
///   function will Err if the path does not exist.
/// - If neither environment variable is set, this function does not verify
///   that the directory exists.
pub(crate) fn find_codex_home() -> std::io::Result<PathBuf> {
    // Honor `CODEXEL_HOME` (preferred) and `CODEX_HOME` (legacy) when set to
    // allow users (and tests) to override the default location.
    if let Ok(val) = std::env::var("CODEXEL_HOME")
        && !val.is_empty()
    {
        return PathBuf::from(val).canonicalize();
    }

    if let Ok(val) = std::env::var("CODEX_HOME")
        && !val.is_empty()
    {
        return PathBuf::from(val).canonicalize();
    }

    let home = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;

    let codexel_home = home.join(".codexel");
    Ok(codexel_home)
}
