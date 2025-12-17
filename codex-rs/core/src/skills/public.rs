use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use thiserror::Error;

const PUBLIC_SKILLS_REPO_URL: &str = "https://github.com/openai/skills.git";
const PUBLIC_SKILLS_DIR_NAME: &str = ".public";
const SKILLS_DIR_NAME: &str = "skills";

pub(crate) fn public_cache_root_dir(codex_home: &Path) -> PathBuf {
    codex_home
        .join(SKILLS_DIR_NAME)
        .join(PUBLIC_SKILLS_DIR_NAME)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublicSkillsRefreshOutcome {
    Skipped,
    Updated,
}

impl PublicSkillsRefreshOutcome {
    pub(crate) fn updated(self) -> bool {
        matches!(self, Self::Updated)
    }
}

pub(crate) fn refresh_public_skills(
    codex_home: &Path,
) -> Result<PublicSkillsRefreshOutcome, PublicSkillsError> {
    // Keep tests deterministic and offline-safe. Tests that want to exercise the
    // refresh behavior should call `refresh_public_skills_from_repo_url`.
    if cfg!(test) {
        return Ok(PublicSkillsRefreshOutcome::Skipped);
    }
    refresh_public_skills_inner(codex_home, PUBLIC_SKILLS_REPO_URL)
}

#[cfg(test)]
pub(crate) fn refresh_public_skills_from_repo_url(
    codex_home: &Path,
    repo_url: &str,
) -> Result<PublicSkillsRefreshOutcome, PublicSkillsError> {
    refresh_public_skills_inner(codex_home, repo_url)
}

fn refresh_public_skills_inner(
    codex_home: &Path,
    repo_url: &str,
) -> Result<PublicSkillsRefreshOutcome, PublicSkillsError> {
    // Best-effort refresh: clone the repo to a temp dir, stage its `skills/`, then atomically swap
    // the staged directory into the public cache.
    let skills_root_dir = codex_home.join(SKILLS_DIR_NAME);
    fs::create_dir_all(&skills_root_dir)
        .map_err(|source| PublicSkillsError::io("create skills root dir", source))?;

    let dest_public = public_cache_root_dir(codex_home);

    let tmp_dir = skills_root_dir.join(format!(".public-tmp-{}", rand_suffix()));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir).map_err(|source| {
            PublicSkillsError::io("remove existing public skills tmp dir", source)
        })?;
    }
    fs::create_dir_all(&tmp_dir)
        .map_err(|source| PublicSkillsError::io("create public skills tmp dir", source))?;

    let checkout_dir = tmp_dir.join("checkout");
    clone_repo(repo_url, &checkout_dir)?;

    let src_skills = checkout_dir.join(SKILLS_DIR_NAME);
    let src_skills_metadata = fs::symlink_metadata(&src_skills)
        .map_err(|source| PublicSkillsError::io("read skills dir metadata", source))?;
    let src_skills_type = src_skills_metadata.file_type();
    if src_skills_type.is_symlink() || !src_skills_type.is_dir() {
        return Err(PublicSkillsError::RepoMissingSkillsDir {
            skills_dir_name: SKILLS_DIR_NAME,
        });
    }

    let staged_public = tmp_dir.join(PUBLIC_SKILLS_DIR_NAME);
    stage_skills_dir(&src_skills, &staged_public)?;

    atomic_swap_dir(&staged_public, &dest_public, &skills_root_dir)?;

    fs::remove_dir_all(&tmp_dir)
        .map_err(|source| PublicSkillsError::io("remove public skills tmp dir", source))?;
    Ok(PublicSkillsRefreshOutcome::Updated)
}

fn stage_skills_dir(src: &Path, staged: &Path) -> Result<(), PublicSkillsError> {
    fs::rename(src, staged).map_err(|source| PublicSkillsError::io("stage skills dir", source))?;

    prune_symlinks_and_special_files(staged)?;
    Ok(())
}

fn prune_symlinks_and_special_files(root: &Path) -> Result<(), PublicSkillsError> {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)
            .map_err(|source| PublicSkillsError::io("read staged skills dir", source))?
        {
            let entry = entry
                .map_err(|source| PublicSkillsError::io("read staged skills dir entry", source))?;
            let file_type = entry
                .file_type()
                .map_err(|source| PublicSkillsError::io("read staged skills entry type", source))?;
            let path = entry.path();

            if file_type.is_symlink() {
                fs::remove_file(&path).map_err(|source| {
                    PublicSkillsError::io("remove symlink from staged skills", source)
                })?;
                continue;
            }

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if file_type.is_file() {
                continue;
            }

            fs::remove_file(&path).map_err(|source| {
                PublicSkillsError::io("remove special file from staged skills", source)
            })?;
        }
    }

    Ok(())
}

fn clone_repo(repo_url: &str, checkout_dir: &Path) -> Result<(), PublicSkillsError> {
    let out = std::process::Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "true")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(repo_url)
        .arg(checkout_dir)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|source| PublicSkillsError::io("spawn `git clone`", source))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr = stderr.trim();
        return if stderr.is_empty() {
            Err(PublicSkillsError::GitCloneFailed { status: out.status })
        } else {
            Err(PublicSkillsError::GitCloneFailedWithStderr {
                status: out.status,
                stderr: stderr.to_owned(),
            })
        };
    }
    Ok(())
}

fn atomic_swap_dir(staged: &Path, dest: &Path, parent: &Path) -> Result<(), PublicSkillsError> {
    if let Some(dest_parent) = dest.parent() {
        fs::create_dir_all(dest_parent)
            .map_err(|source| PublicSkillsError::io("create public skills dest parent", source))?;
    }

    let backup_base = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skills");
    let backup = parent.join(format!("{backup_base}.old-{}", rand_suffix()));
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|source| PublicSkillsError::io("remove old public skills backup", source))?;
    }

    if dest.exists() {
        fs::rename(dest, &backup)
            .map_err(|source| PublicSkillsError::io("rename public skills to backup", source))?;
    }

    if let Err(err) = fs::rename(staged, dest) {
        if backup.exists() {
            let _ = fs::rename(&backup, dest);
        }
        return Err(PublicSkillsError::io(
            "rename staged public skills into place",
            err,
        ));
    }

    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|source| PublicSkillsError::io("remove public skills backup", source))?;
    }

    Ok(())
}

fn rand_suffix() -> String {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{pid:x}-{nanos:x}")
}

#[derive(Debug, Error)]
pub(crate) enum PublicSkillsError {
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("repo did not contain a `{skills_dir_name}` directory")]
    RepoMissingSkillsDir { skills_dir_name: &'static str },

    #[error("`git clone` failed with status {status}")]
    GitCloneFailed { status: ExitStatus },

    #[error("`git clone` failed with status {status}: {stderr}")]
    GitCloneFailedWithStderr { status: ExitStatus, stderr: String },
}

impl PublicSkillsError {
    fn io(action: &'static str, source: std::io::Error) -> Self {
        Self::Io { action, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn write_public_skill(repo_dir: &TempDir, name: &str, description: &str) {
        let skills_dir = repo_dir.path().join("skills").join(name);
        fs::create_dir_all(&skills_dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
        fs::write(skills_dir.join("SKILL.md"), content).unwrap();
    }

    fn git(repo_dir: &TempDir, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=codex-test",
                "-c",
                "user.email=codex-test@example.com",
            ])
            .args(args)
            .current_dir(repo_dir.path())
            .status()
            .unwrap();
        assert!(status.success(), "git command failed: {args:?}");
    }

    #[tokio::test]
    async fn refresh_copies_skills_subdir_into_public_cache() {
        let codex_home = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        git(&repo_dir, &["init"]);
        write_public_skill(&repo_dir, "demo", "from repo");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "init"]);

        refresh_public_skills_from_repo_url(codex_home.path(), repo_dir.path().to_str().unwrap())
            .unwrap();

        let path = public_cache_root_dir(codex_home.path())
            .join("demo")
            .join("SKILL.md");
        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("name: demo"));
        assert!(contents.contains("description: from repo"));
    }

    #[tokio::test]
    async fn refresh_overwrites_existing_public_cache() {
        let codex_home = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        git(&repo_dir, &["init"]);
        write_public_skill(&repo_dir, "demo", "v1");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "v1"]);

        refresh_public_skills_from_repo_url(codex_home.path(), repo_dir.path().to_str().unwrap())
            .unwrap();

        write_public_skill(&repo_dir, "demo", "v2");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "v2"]);

        refresh_public_skills_from_repo_url(codex_home.path(), repo_dir.path().to_str().unwrap())
            .unwrap();

        let path = public_cache_root_dir(codex_home.path())
            .join("demo")
            .join("SKILL.md");
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents.matches("description:").count(), 1);
        assert!(contents.contains("description: v2"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refresh_prunes_symlinks_inside_skills_dir() {
        use std::os::unix::fs::symlink;

        let codex_home = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        git(&repo_dir, &["init"]);
        write_public_skill(&repo_dir, "demo", "from repo");

        let demo_dir = repo_dir.path().join("skills").join("demo");
        symlink("SKILL.md", demo_dir.join("link-to-skill")).unwrap();
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "init"]);

        refresh_public_skills_from_repo_url(codex_home.path(), repo_dir.path().to_str().unwrap())
            .unwrap();

        assert!(
            !public_cache_root_dir(codex_home.path())
                .join("demo")
                .join("link-to-skill")
                .exists()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refresh_rejects_symlinked_skills_dir() {
        use std::os::unix::fs::symlink;

        let codex_home = tempfile::tempdir().unwrap();
        let repo_dir = tempfile::tempdir().unwrap();
        git(&repo_dir, &["init"]);

        let skills_target = repo_dir.path().join("skills-target");
        fs::create_dir_all(skills_target.join("demo")).unwrap();
        fs::write(
            skills_target.join("demo").join("SKILL.md"),
            "---\nname: demo\ndescription: from repo\n---\n",
        )
        .unwrap();
        symlink("skills-target", repo_dir.path().join("skills")).unwrap();
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "init"]);

        let err = refresh_public_skills_from_repo_url(
            codex_home.path(),
            repo_dir.path().to_str().unwrap(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("repo did not contain"));
    }
}
