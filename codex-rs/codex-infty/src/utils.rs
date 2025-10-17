use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;

pub fn trim_to_non_empty(opt: Option<String>) -> Option<String> {
    opt.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn required_trimmed(opt: Option<String>, err_msg: &str) -> Result<String> {
    trim_to_non_empty(opt).ok_or_else(|| anyhow!(err_msg.to_string()))
}

pub fn resolve_deliverable_path(base: &Path, candidate: &str) -> Result<PathBuf> {
    let base_abs = base
        .canonicalize()
        .with_context(|| format!("failed to canonicalize run store {}", base.display()))?;

    let candidate_path = Path::new(candidate);
    let joined = if candidate_path.is_absolute() {
        candidate_path.to_path_buf()
    } else {
        base_abs.join(candidate_path)
    };

    let resolved = joined.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize deliverable path {}",
            joined.display()
        )
    })?;

    if !resolved.starts_with(&base_abs) {
        bail!(
            "deliverable path {} escapes run store {}",
            resolved.display(),
            base_abs.display()
        );
    }

    Ok(resolved)
}

pub fn objective_as_str(options: &crate::types::RunExecutionOptions) -> Option<&str> {
    options
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_deliverable_within_base() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        std::fs::create_dir_all(base.join("deliverable")).unwrap();
        std::fs::write(base.join("deliverable").join("a.txt"), "ok").unwrap();
        let resolved = resolve_deliverable_path(base, "deliverable/a.txt").unwrap();
        let base_abs = base.canonicalize().unwrap();
        assert!(resolved.starts_with(&base_abs));
    }

    #[test]
    fn resolve_deliverable_rejects_escape() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        // Create a real file outside of base so canonicalization succeeds
        let outside = TempDir::new().unwrap();
        let outside_file = outside.path().join("outside.txt");
        std::fs::write(&outside_file, "nope").unwrap();

        let err = resolve_deliverable_path(base, outside_file.to_str().unwrap()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("escapes run store"));
    }
}
