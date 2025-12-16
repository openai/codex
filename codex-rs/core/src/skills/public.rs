use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

// TODO: Temporary test repo. Revert to `https://github.com/openai/skills.git`.
const PUBLIC_SKILLS_REPO_URL: &str = "https://github.com/xl-openai/test.git";
const PUBLIC_SKILLS_DIR_NAME: &str = ".public";
const SKILLS_DIR_NAME: &str = "skills";
const PUBLIC_SKILLS_LOCK_FILENAME: &str = ".lock";

fn public_cache_root_dir(codex_home: &Path) -> PathBuf {
    codex_home
        .join(SKILLS_DIR_NAME)
        .join(PUBLIC_SKILLS_DIR_NAME)
}

pub(crate) fn public_skills_dir(codex_home: &Path) -> PathBuf {
    public_cache_root_dir(codex_home).join(SKILLS_DIR_NAME)
}

pub(crate) fn refresh_public_skills_blocking(codex_home: &Path) -> anyhow::Result<()> {
    // Keep tests deterministic and offline-safe. Tests that want to exercise the
    // refresh behavior should call `refresh_public_skills_from_repo_url_blocking`.
    if cfg!(test) {
        return Ok(());
    }
    refresh_public_skills_inner_blocking(codex_home, PUBLIC_SKILLS_REPO_URL)
}

#[cfg(test)]
pub(crate) fn refresh_public_skills_from_repo_url_blocking(
    codex_home: &Path,
    repo_url: &str,
) -> anyhow::Result<()> {
    refresh_public_skills_inner_blocking(codex_home, repo_url)
}

fn refresh_public_skills_inner_blocking(codex_home: &Path, repo_url: &str) -> anyhow::Result<()> {
    // Best-effort refresh: clone the repo to a temp dir, copy its `skills/`, then atomically swap
    // the staged directory into the public cache.
    let public_dir = public_cache_root_dir(codex_home);
    fs::create_dir_all(&public_dir)?;
    let lock_path = public_dir.join(PUBLIC_SKILLS_LOCK_FILENAME);
    let _lock = match PublicSkillsLock::acquire(&lock_path) {
        Ok(lock) => lock,
        Err(LockError::Locked) => {
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let tmp_dir = public_dir.join(format!(".tmp-{}", rand_suffix()));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let checkout_dir = tmp_dir.join("checkout");
    clone_repo_blocking(repo_url, &checkout_dir)?;

    let src_skills = checkout_dir.join(SKILLS_DIR_NAME);
    if !src_skills.is_dir() {
        return Err(anyhow::anyhow!(
            "repo did not contain a `{SKILLS_DIR_NAME}` directory"
        ));
    }

    let staged_skills = tmp_dir.join(SKILLS_DIR_NAME);
    copy_dir_recursive(&src_skills, &staged_skills)?;

    let dest_skills = public_skills_dir(codex_home);
    atomic_swap_dir(&staged_skills, &dest_skills, &public_dir)?;

    fs::remove_dir_all(&tmp_dir)?;
    Ok(())
}

fn clone_repo_blocking(repo_url: &str, checkout_dir: &Path) -> anyhow::Result<()> {
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
        .map_err(|err| anyhow::anyhow!("failed to spawn `git clone`: {err}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            return Err(anyhow::anyhow!(
                "`git clone` failed with status {}",
                out.status
            ));
        }
        return Err(anyhow::anyhow!(
            "`git clone` failed with status {}: {stderr}",
            out.status
        ));
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;

    let mut stack: Vec<PathBuf> = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            let rel = path.strip_prefix(src)?;
            let out_path = dest.join(rel);

            if file_type.is_dir() {
                fs::create_dir_all(&out_path)?;
                stack.push(path);
            } else if file_type.is_file() {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&path, &out_path)?;
            }
        }
    }

    Ok(())
}

fn atomic_swap_dir(staged: &Path, dest: &Path, parent: &Path) -> anyhow::Result<()> {
    if let Some(dest_parent) = dest.parent() {
        fs::create_dir_all(dest_parent)?;
    }

    let backup = parent.join(format!("skills.old-{}", rand_suffix()));
    if backup.exists() {
        fs::remove_dir_all(&backup)?;
    }

    if dest.exists() {
        fs::rename(dest, &backup)?;
    }

    if let Err(err) = fs::rename(staged, dest) {
        if backup.exists() {
            let _ = fs::rename(&backup, dest);
        }
        return Err(err.into());
    }

    if backup.exists() {
        fs::remove_dir_all(&backup)?;
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

enum LockError {
    Locked,
    Io(std::io::Error),
}

impl From<LockError> for anyhow::Error {
    fn from(value: LockError) -> Self {
        match value {
            LockError::Locked => anyhow::anyhow!("lock already held"),
            LockError::Io(err) => anyhow::Error::from(err),
        }
    }
}

struct PublicSkillsLock {
    path: PathBuf,
}

impl PublicSkillsLock {
    fn acquire(path: &Path) -> Result<Self, LockError> {
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
        {
            Ok(mut file) => {
                let _ = writeln!(&mut file, "pid={}", std::process::id());
                Ok(Self {
                    path: path.to_path_buf(),
                })
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Err(LockError::Locked),
            Err(err) => Err(LockError::Io(err)),
        }
    }
}

impl Drop for PublicSkillsLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
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

        refresh_public_skills_from_repo_url_blocking(
            codex_home.path(),
            repo_dir.path().to_str().unwrap(),
        )
        .unwrap();

        let path = public_skills_dir(codex_home.path())
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

        refresh_public_skills_from_repo_url_blocking(
            codex_home.path(),
            repo_dir.path().to_str().unwrap(),
        )
        .unwrap();

        write_public_skill(&repo_dir, "demo", "v2");
        git(&repo_dir, &["add", "."]);
        git(&repo_dir, &["commit", "-m", "v2"]);

        refresh_public_skills_from_repo_url_blocking(
            codex_home.path(),
            repo_dir.path().to_str().unwrap(),
        )
        .unwrap();

        let path = public_skills_dir(codex_home.path())
            .join("demo")
            .join("SKILL.md");
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents.matches("description:").count(), 1);
        assert!(contents.contains("description: v2"));
    }
}
