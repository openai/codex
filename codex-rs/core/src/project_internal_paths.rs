use std::ffi::OsStr;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

pub(crate) const PROJECT_INTERNAL_DIR_NAME: &str = ".codexel";
pub(crate) const APPROVED_PLAN_MARKDOWN_FILENAME: &str = "plan.md";

pub(crate) fn project_internal_dir(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_INTERNAL_DIR_NAME)
}

pub(crate) fn approved_plan_markdown_path(cwd: &Path) -> PathBuf {
    project_internal_dir(cwd).join(APPROVED_PLAN_MARKDOWN_FILENAME)
}

pub(crate) fn is_path_in_project_internal_dir(path: &Path, cwd: &Path) -> bool {
    let normalized_cwd = normalize_path(cwd);
    let normalized_path = normalize_path(path);
    if is_project_internal_relative_path(&normalized_path, &normalized_cwd) {
        return true;
    }

    let canonical_cwd = dunce::canonicalize(cwd);
    let canonical_path = dunce::canonicalize(path);
    if let (Ok(canonical_cwd), Ok(canonical_path)) = (canonical_cwd, canonical_path) {
        let internal_dir = canonical_cwd.join(PROJECT_INTERNAL_DIR_NAME);
        return canonical_path.starts_with(internal_dir);
    }

    false
}

fn is_project_internal_relative_path(path: &Path, cwd: &Path) -> bool {
    let relative = match path.strip_prefix(cwd) {
        Ok(relative) => relative,
        Err(_) => return false,
    };
    relative
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == OsStr::new(PROJECT_INTERNAL_DIR_NAME))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn internal_path_detected_lexically() {
        let temp = TempDir::new().expect("temp dir");
        let cwd = temp.path();

        let target = cwd.join(".codexel").join("plan.md");
        assert!(is_path_in_project_internal_dir(&target, cwd));
    }

    #[test]
    fn non_internal_path_not_detected() {
        let temp = TempDir::new().expect("temp dir");
        let cwd = temp.path();

        let target = cwd.join("src").join("main.rs");
        assert!(!is_path_in_project_internal_dir(&target, cwd));
    }
}
