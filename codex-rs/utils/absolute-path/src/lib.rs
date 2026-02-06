use dirs::home_dir;
use path_absolutize::Absolutize;
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

/// A path that is guaranteed to be absolute and normalized (though it is not
/// guaranteed to be canonicalized or exist on the filesystem).
///
/// IMPORTANT: When deserializing an `AbsolutePathBuf`, a base path must be set
/// using [AbsolutePathBufGuard::new]. If no base path is set, the
/// deserialization will fail unless the path being deserialized is already
/// absolute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, TS)]
pub struct AbsolutePathBuf(PathBuf);

impl AbsolutePathBuf {
    fn maybe_expand_home_directory(path: &Path) -> PathBuf {
        let Some(path_str) = path.to_str() else {
            return path.to_path_buf();
        };
        if cfg!(not(target_os = "windows"))
            && let Some(home) = home_dir()
        {
            if path_str == "~" {
                return home;
            }
            if let Some(rest) = path_str.strip_prefix("~/") {
                let rest = rest.trim_start_matches('/');
                if rest.is_empty() {
                    return home;
                }
                return home.join(rest);
            }
        }
        path.to_path_buf()
    }

    pub fn resolve_path_against_base<P: AsRef<Path>, B: AsRef<Path>>(
        path: P,
        base_path: B,
    ) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        let absolute_path = expanded.absolutize_from(base_path.as_ref())?;
        Ok(Self(absolute_path.into_owned()))
    }

    pub fn from_absolute_path<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        let absolute_path = expanded.absolutize()?;
        Ok(Self(absolute_path.into_owned()))
    }

    pub fn current_dir() -> std::io::Result<Self> {
        let current_dir = std::env::current_dir()?;
        Self::from_absolute_path(current_dir)
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Self> {
        Self::resolve_path_against_base(path, &self.0)
    }

    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|p| {
            #[expect(clippy::expect_used)]
            Self::from_absolute_path(p).expect("parent of AbsolutePathBuf must be absolute")
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

impl AsRef<Path> for AbsolutePathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl From<AbsolutePathBuf> for PathBuf {
    fn from(path: AbsolutePathBuf) -> Self {
        path.into_path_buf()
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
            Some(base) => {
                Ok(Self::resolve_path_against_base(path, base).map_err(SerdeError::custom)?)
            }
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
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn create_with_absolute_path_ignores_base_path() {
        let base_dir = tempdir().expect("base dir");
        let absolute_dir = tempdir().expect("absolute dir");
        let base_path = base_dir.path();
        let absolute_path = absolute_dir.path().join("file.txt");
        let abs_path_buf =
            AbsolutePathBuf::resolve_path_against_base(absolute_path.clone(), base_path)
                .expect("failed to create");
        assert_eq!(abs_path_buf.as_path(), absolute_path.as_path());
    }

    #[test]
    fn relative_path_is_resolved_against_base_path() {
        let temp_dir = tempdir().expect("base dir");
        let base_dir = temp_dir.path();
        let abs_path_buf = AbsolutePathBuf::resolve_path_against_base("file.txt", base_dir)
            .expect("failed to create");
        assert_eq!(abs_path_buf.as_path(), base_dir.join("file.txt").as_path());
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn home_directory_root_on_non_windows_is_expanded_in_deserialization() {
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn home_directory_subpath_on_non_windows_is_expanded_in_deserialization() {
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn home_directory_double_slash_on_non_windows_is_expanded_in_deserialization() {
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

    #[cfg(target_os = "windows")]
    #[test]
    fn home_directory_on_windows_is_not_expanded_in_deserialization() {
        let temp_dir = tempdir().expect("base dir");
        let base_dir = temp_dir.path();
        let abs_path_buf = {
            let _guard = AbsolutePathBufGuard::new(base_dir);
            serde_json::from_str::<AbsolutePathBuf>("\"~/code\"").expect("failed to deserialize")
        };
        assert_eq!(
            abs_path_buf.as_path(),
            base_dir.join("~").join("code").as_path()
        );
    }
}
