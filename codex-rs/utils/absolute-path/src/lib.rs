use path_absolutize::Absolutize;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;
use std::cell::RefCell;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;

/// A path that is guaranteed to be absolute and normalized (though it is not
/// guaranteed to be canonicalized or exist on the filesystem).
///
/// IMPORTANT: When deserializing an `AbsolutePathBuf`, a base path must be set
/// using `AbsolutePathBufGuard::new(base_path)`. If no base path is set, the
/// deserialization will fail unless the path being deserialized is already
/// absolute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AbsolutePathBuf(PathBuf);

impl AbsolutePathBuf {
    pub fn resolve_path_against_base<P, B>(path: P, base_path: B) -> std::io::Result<Self>
    where
        P: AsRef<Path>,
        B: AsRef<Path>,
    {
        let absolute_path = path.as_ref().absolutize_from(base_path.as_ref())?;
        Ok(Self(absolute_path.into_owned()))
    }

    pub fn from_absolute_path<P>(path: P) -> std::io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let absolute_path = path.as_ref().absolutize()?;
        Ok(Self(absolute_path.into_owned()))
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

    pub fn display(&self) -> Display<'_> {
        self.0.display()
    }
}

thread_local! {
    static ABSOLUTE_PATH_BASE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

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

impl AsRef<Path> for AbsolutePathBuf {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<AbsolutePathBuf> for PathBuf {
    fn from(path: AbsolutePathBuf) -> Self {
        path.into_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
