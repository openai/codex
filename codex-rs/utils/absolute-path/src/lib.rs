use dirs::home_dir;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;
use std::cell::RefCell;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;
use ts_rs::TS;

mod absolutize;

/// A path that is guaranteed to be absolute and normalized (though it is not
/// guaranteed to be canonicalized or exist on the filesystem).
///
/// IMPORTANT: When deserializing an `AbsolutePathBuf`, a base path must be set
/// using [AbsolutePathBufGuard::new]. If no base path is set, the
/// deserialization will fail unless the path being deserialized is already
/// absolute.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema, TS)]
pub struct AbsolutePathBuf(PathBuf);

impl AbsolutePathBuf {
    fn maybe_expand_home_directory(path: &Path) -> PathBuf {
        if let Some(path_str) = path.to_str()
            && let Some(home) = home_dir()
            && let Some(rest) = path_str.strip_prefix('~')
        {
            if rest.is_empty() {
                return home;
            } else if let Some(rest) = rest.strip_prefix('/') {
                return home.join(rest.trim_start_matches('/'));
            } else if cfg!(windows)
                && let Some(rest) = rest.strip_prefix('\\')
            {
                return home.join(rest.trim_start_matches('\\'));
            }
        }
        path.to_path_buf()
    }

    pub fn resolve_path_against_base<P: AsRef<Path>, B: AsRef<Path>>(
        path: P,
        base_path: B,
    ) -> Self {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        Self(absolutize::absolutize_from(&expanded, base_path.as_ref()))
    }

    pub fn from_absolute_path<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        Ok(Self(absolutize::absolutize(&expanded)?))
    }

    pub fn from_absolute_path_checked<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        if !expanded.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("path is not absolute: {}", path.as_ref().display()),
            ));
        }

        Ok(Self(absolutize::absolutize_from(&expanded, Path::new("/"))))
    }

    pub fn current_dir() -> std::io::Result<Self> {
        let current_dir = std::env::current_dir()?;
        Ok(Self(absolutize::absolutize_from(
            &current_dir,
            &current_dir,
        )))
    }

    /// Construct an absolute path from `path`, resolving relative paths against
    /// the process current working directory.
    pub fn relative_to_current_dir<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(Self::resolve_path_against_base(
            path,
            std::env::current_dir()?,
        ))
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        Self::resolve_path_against_base(path, &self.0)
    }

    pub fn canonicalize(&self) -> std::io::Result<Self> {
        dunce::canonicalize(&self.0).map(Self)
    }

    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|p| {
            debug_assert!(
                p.is_absolute(),
                "parent of AbsolutePathBuf must be absolute"
            );
            Self(p.to_path_buf())
        })
    }

    pub fn ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        self.0.ancestors().map(|p| {
            debug_assert!(
                p.is_absolute(),
                "ancestor of AbsolutePathBuf must be absolute"
            );
            Self(p.to_path_buf())
        })
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }

    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    pub fn display(&self) -> Display<'_> {
        self.0.display()
    }
}

/// Canonicalize a path when possible, but preserve the logical absolute path
/// whenever canonicalization would rewrite it through a nested symlink.
///
/// Top-level system aliases such as macOS `/var -> /private/var` still remain
/// canonicalized so existing runtime expectations around those paths stay
/// stable. If the full path cannot be canonicalized, this returns the logical
/// absolute path; use [`canonicalize_existing_preserving_symlinks`] for paths
/// that must exist.
pub fn canonicalize_preserving_symlinks(path: &Path) -> std::io::Result<PathBuf> {
    let logical = AbsolutePathBuf::from_absolute_path(path)?.into_path_buf();
    let preserve_logical_path = should_preserve_logical_path(&logical);
    match dunce::canonicalize(path) {
        Ok(canonical) if preserve_logical_path && canonical != logical => Ok(logical),
        Ok(canonical) => Ok(canonical),
        Err(_) => Ok(logical),
    }
}

/// Canonicalize an existing path while preserving the logical absolute path
/// whenever canonicalization would rewrite it through a nested symlink.
///
/// Unlike [`canonicalize_preserving_symlinks`], canonicalization failures are
/// propagated so callers can reject invalid working directories early.
pub fn canonicalize_existing_preserving_symlinks(path: &Path) -> std::io::Result<PathBuf> {
    let logical = AbsolutePathBuf::from_absolute_path(path)?.into_path_buf();
    let canonical = dunce::canonicalize(path)?;
    if should_preserve_logical_path(&logical) && canonical != logical {
        Ok(logical)
    } else {
        Ok(canonical)
    }
}

fn should_preserve_logical_path(logical: &Path) -> bool {
    logical.ancestors().any(|ancestor| {
        let Ok(metadata) = std::fs::symlink_metadata(ancestor) else {
            return false;
        };
        metadata.file_type().is_symlink() && ancestor.parent().and_then(Path::parent).is_some()
    })
}

impl AsRef<Path> for AbsolutePathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for AbsolutePathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AbsolutePathBuf> for PathBuf {
    fn from(path: AbsolutePathBuf) -> Self {
        path.into_path_buf()
    }
}

/// Helpers for constructing absolute paths in tests.
pub mod test_support {
    use super::AbsolutePathBuf;
    use std::path::Path;
    use std::path::PathBuf;

    /// Creates a platform-absolute [`PathBuf`] from a Unix-style absolute test path.
    ///
    /// On Windows, `/tmp/example` maps to `C:\tmp\example`.
    pub fn test_path_buf(unix_path: &str) -> PathBuf {
        if cfg!(windows) {
            let mut path = PathBuf::from(r"C:\");
            path.extend(
                unix_path
                    .trim_start_matches('/')
                    .split('/')
                    .filter(|segment| !segment.is_empty()),
            );
            path
        } else {
            PathBuf::from(unix_path)
        }
    }

    /// Extension methods for converting paths into [`AbsolutePathBuf`] values in tests.
    pub trait PathExt {
        /// Converts an already absolute path into an [`AbsolutePathBuf`].
        fn abs(&self) -> AbsolutePathBuf;
    }

    impl PathExt for Path {
        #[expect(clippy::expect_used)]
        fn abs(&self) -> AbsolutePathBuf {
            AbsolutePathBuf::from_absolute_path_checked(self)
                .expect("path should already be absolute")
        }
    }

    /// Extension methods for converting path buffers into [`AbsolutePathBuf`] values in tests.
    pub trait PathBufExt {
        /// Converts an already absolute path buffer into an [`AbsolutePathBuf`].
        fn abs(&self) -> AbsolutePathBuf;
    }

    impl PathBufExt for PathBuf {
        fn abs(&self) -> AbsolutePathBuf {
            self.as_path().abs()
        }
    }
}

impl TryFrom<&Path> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<PathBuf> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<&str> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<String> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

thread_local! {
    static ABSOLUTE_PATH_BASE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Ensure this guard is held while deserializing `AbsolutePathBuf` values to
/// provide a base path for resolving relative paths. Because this relies on
/// thread-local storage, the deserialization must be single-threaded and
/// occur on the same thread that created the guard.
pub struct AbsolutePathBufGuard;

impl AbsolutePathBufGuard {
    pub fn new(base_path: &Path) -> Self {
        ABSOLUTE_PATH_BASE.with(|cell| {
            *cell.borrow_mut() = Some(base_path.to_path_buf());
        });
        Self
    }
}

impl Drop for AbsolutePathBufGuard {
    fn drop(&mut self) {
        ABSOLUTE_PATH_BASE.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

impl<'de> Deserialize<'de> for AbsolutePathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        ABSOLUTE_PATH_BASE.with(|cell| match cell.borrow().as_deref() {
            Some(base) => Ok(Self::resolve_path_against_base(path, base)),
            None if path.is_absolute() => {
                Self::from_absolute_path(path).map_err(SerdeError::custom)
            }
            None => Err(SerdeError::custom(
                "AbsolutePathBuf deserialized without a base path",
            )),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn create_with_absolute_path_ignores_base_path() {
        let base_dir = tempdir().expect("base dir");
        let absolute_dir = tempdir().expect("absolute dir");
        let base_path = base_dir.path();
        let absolute_path = absolute_dir.path().join("file.txt");
        let abs_path_buf =
            AbsolutePathBuf::resolve_path_against_base(absolute_path.clone(), base_path);
        assert_eq!(abs_path_buf.as_path(), absolute_path.as_path());
    }

    #[test]
    fn from_absolute_path_checked_rejects_relative_path() {
        let err = AbsolutePathBuf::from_absolute_path_checked("relative/path")
            .expect_err("relative path should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn relative_path_is_resolved_against_base_path() {
        let temp_dir = tempdir().expect("base dir");
        let base_dir = temp_dir.path();
        let abs_path_buf = AbsolutePathBuf::resolve_path_against_base("file.txt", base_dir);
        assert_eq!(abs_path_buf.as_path(), base_dir.join("file.txt").as_path());
    }

    #[test]
    fn relative_path_dots_are_normalized_against_base_path() {
        let temp_dir = tempdir().expect("base dir");
        let base_dir = temp_dir.path();
        let abs_path_buf =
            AbsolutePathBuf::resolve_path_against_base("./nested/../file.txt", base_dir);
        assert_eq!(abs_path_buf.as_path(), base_dir.join("file.txt").as_path());
    }

    #[test]
    fn canonicalize_returns_absolute_path_buf() {
        let temp_dir = tempdir().expect("base dir");
        fs::create_dir(temp_dir.path().join("one")).expect("create one dir");
        fs::create_dir(temp_dir.path().join("two")).expect("create two dir");
        fs::write(temp_dir.path().join("two").join("file.txt"), "").expect("write file");
        let abs_path_buf =
            AbsolutePathBuf::from_absolute_path(temp_dir.path().join("one/../two/./file.txt"))
                .expect("absolute path");
        assert_eq!(
            abs_path_buf
                .canonicalize()
                .expect("path should canonicalize")
                .as_path(),
            dunce::canonicalize(temp_dir.path().join("two").join("file.txt"))
                .expect("expected path should canonicalize")
                .as_path()
        );
    }

    #[test]
    fn canonicalize_returns_error_for_missing_path() {
        let temp_dir = tempdir().expect("base dir");
        let abs_path_buf = AbsolutePathBuf::from_absolute_path(temp_dir.path().join("missing.txt"))
            .expect("absolute path");

        assert!(abs_path_buf.canonicalize().is_err());
    }

    #[test]
    fn ancestors_returns_absolute_path_bufs() {
        let abs_path_buf =
            AbsolutePathBuf::from_absolute_path_checked(test_path_buf("/tmp/one/two"))
                .expect("absolute path");

        let ancestors = abs_path_buf
            .ancestors()
            .map(|path| path.to_path_buf())
            .collect::<Vec<_>>();

        let expected = vec![
            test_path_buf("/tmp/one/two"),
            test_path_buf("/tmp/one"),
            test_path_buf("/tmp"),
            test_path_buf("/"),
        ];

        assert_eq!(ancestors, expected);
    }

    #[test]
    fn relative_to_current_dir_resolves_relative_path() -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let abs_path_buf = AbsolutePathBuf::relative_to_current_dir("file.txt")?;
        assert_eq!(
            abs_path_buf.as_path(),
            current_dir.join("file.txt").as_path()
        );
        Ok(())
    }

    #[test]
    fn guard_used_in_deserialization() {
        let temp_dir = tempdir().expect("base dir");
        let base_dir = temp_dir.path();
        let relative_path = "subdir/file.txt";
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(base_dir);
            serde_json::from_str::<AbsolutePathBuf>(&format!(r#""{relative_path}""#))
                .expect("failed to deserialize")
        };
        assert_eq!(
            abs_path_buf.as_path(),
            base_dir.join(relative_path).as_path()
        );
    }

    #[test]
    fn home_directory_root_is_expanded_in_deserialization() {
        let Some(home) = home_dir() else {
            return;
        };
        let temp_dir = tempdir().expect("base dir");
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(temp_dir.path());
            serde_json::from_str::<AbsolutePathBuf>("\"~\"").expect("failed to deserialize")
        };
        assert_eq!(abs_path_buf.as_path(), home.as_path());
    }

    #[test]
    fn home_directory_subpath_is_expanded_in_deserialization() {
        let Some(home) = home_dir() else {
            return;
        };
        let temp_dir = tempdir().expect("base dir");
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(temp_dir.path());
            serde_json::from_str::<AbsolutePathBuf>("\"~/code\"").expect("failed to deserialize")
        };
        assert_eq!(abs_path_buf.as_path(), home.join("code").as_path());
    }

    #[test]
    fn home_directory_double_slash_is_expanded_in_deserialization() {
        let Some(home) = home_dir() else {
            return;
        };
        let temp_dir = tempdir().expect("base dir");
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(temp_dir.path());
            serde_json::from_str::<AbsolutePathBuf>("\"~//code\"").expect("failed to deserialize")
        };
        assert_eq!(abs_path_buf.as_path(), home.join("code").as_path());
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_preserving_symlinks_keeps_logical_symlink_path() {
        let temp_dir = tempdir().expect("temp dir");
        let real = temp_dir.path().join("real");
        let link = temp_dir.path().join("link");
        std::fs::create_dir_all(&real).expect("create real dir");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");

        let canonicalized =
            canonicalize_preserving_symlinks(&link).expect("canonicalize preserving symlinks");

        assert_eq!(canonicalized, link);
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_preserving_symlinks_keeps_logical_missing_child_under_symlink() {
        let temp_dir = tempdir().expect("temp dir");
        let real = temp_dir.path().join("real");
        let link = temp_dir.path().join("link");
        std::fs::create_dir_all(&real).expect("create real dir");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");
        let missing = link.join("missing.txt");

        let canonicalized =
            canonicalize_preserving_symlinks(&missing).expect("canonicalize preserving symlinks");

        assert_eq!(canonicalized, missing);
    }

    #[test]
    fn canonicalize_existing_preserving_symlinks_errors_for_missing_path() {
        let temp_dir = tempdir().expect("temp dir");
        let missing = temp_dir.path().join("missing");

        let err = canonicalize_existing_preserving_symlinks(&missing)
            .expect_err("missing path should fail canonicalization");

        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn canonicalize_existing_preserving_symlinks_keeps_logical_symlink_path() {
        let temp_dir = tempdir().expect("temp dir");
        let real = temp_dir.path().join("real");
        let link = temp_dir.path().join("link");
        std::fs::create_dir_all(&real).expect("create real dir");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");

        let canonicalized =
            canonicalize_existing_preserving_symlinks(&link).expect("canonicalize symlink");

        assert_eq!(canonicalized, link);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn home_directory_backslash_subpath_is_expanded_in_deserialization() {
        let Some(home) = home_dir() else {
            return;
        };
        let temp_dir = tempdir().expect("base dir");
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(temp_dir.path());
            let input =
                serde_json::to_string(r#"~\code"#).expect("string should serialize as JSON");
            serde_json::from_str::<AbsolutePathBuf>(&input).expect("is valid abs path")
        };
        assert_eq!(abs_path_buf.as_path(), home.join("code").as_path());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn canonicalize_preserving_symlinks_avoids_verbatim_prefixes() {
        let temp_dir = tempdir().expect("temp dir");

        let canonicalized =
            canonicalize_preserving_symlinks(temp_dir.path()).expect("canonicalize");

        assert_eq!(
            canonicalized,
            dunce::canonicalize(temp_dir.path()).expect("canonicalize temp dir")
        );
        assert!(
            !canonicalized.to_string_lossy().starts_with(r"\\?\"),
            "expected a non-verbatim Windows path, got {canonicalized:?}"
        );
    }
}
