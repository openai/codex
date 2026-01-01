use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartMarkerKind {
    RolloutLineTimestamp,
    RolloutLineIndex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StartMarkerValue {
    Timestamp(String),
    LineIndex(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartMarker {
    pub kind: StartMarkerKind,
    pub value: StartMarkerValue,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitBase {
    pub sha: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutInfo {
    pub filename: String,
    pub start: StartMarker,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoInfo {
    pub cwd: String,
    pub git_base: GitBase,
    pub patch_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Notes {
    pub what_went_wrong: String,
    pub what_good_looks_like: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifacts {
    pub include_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalCaseManifestV0 {
    pub version: String,
    pub case_id: String,
    pub created_at: String,
    pub conversation_id: String,
    pub source: String,
    pub rollout: RolloutInfo,
    pub repo: RepoInfo,
    pub notes: Notes,
    pub artifacts: Artifacts,
}

#[derive(Debug, Clone)]
pub struct CreateEvalCaseArgs {
    pub codex_home: PathBuf,
    pub conversation_id: String,
    pub rollout_path: PathBuf,
    pub start: StartMarker,
    pub repo_cwd: PathBuf,
    /// When present, derive `repo.patch` from the provided commit snapshot instead of the current
    /// working tree.
    pub repo_snapshot: Option<RepoSnapshot>,
    pub notes: Notes,
    pub include_logs: bool,
    pub logs_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSnapshot {
    pub base_sha: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone)]
pub struct CreateEvalCaseResult {
    pub case_id: String,
    pub path: PathBuf,
}

fn sanitize_repo_slug(input: &str) -> String {
    // Keep it short + filesystem-friendly.
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
            continue;
        }
        if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "repo".to_string()
    } else {
        out
    }
}

fn repo_slug(repo_cwd: &Path) -> String {
    // Prefer the git repo root name, but fall back to the cwd basename.
    let top_level = git_stdout(repo_cwd, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from);
    let basename = top_level
        .as_ref()
        .and_then(|p| p.file_name())
        .or_else(|| repo_cwd.file_name());
    let basename = basename
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    sanitize_repo_slug(&basename)
}

pub fn create_eval_case_bundle(args: &CreateEvalCaseArgs) -> anyhow::Result<CreateEvalCaseResult> {
    let created_at = OffsetDateTime::now_utc();
    let created_at_rfc3339 = created_at.format(&Rfc3339).context("format created_at")?;

    let ts_for_id = format!(
        "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}",
        created_at.year(),
        u8::from(created_at.month()),
        created_at.day(),
        created_at.hour(),
        created_at.minute(),
        created_at.second()
    );

    // Short, human-scannable id: datetime + repo + 6-digit suffix.
    // Collision risk is low and acceptable for local bundles.
    let repo = repo_slug(&args.repo_cwd);
    let id6 = (Uuid::new_v4().as_u128() % 1_000_000) as u32;
    let case_id = format!("{ts_for_id}-{repo}-{id6:06}");

    let bundle_dir = args.codex_home.join("eval-case").join(&case_id);
    std::fs::create_dir_all(&bundle_dir)
        .with_context(|| format!("create eval bundle dir {}", bundle_dir.display()))?;

    let rollout_dst = bundle_dir.join("rollout.jsonl");
    std::fs::copy(&args.rollout_path, &rollout_dst).with_context(|| {
        format!(
            "copy rollout {} -> {}",
            args.rollout_path.display(),
            rollout_dst.display()
        )
    })?;

    let (base_sha, patch) = match args.repo_snapshot.as_ref() {
        Some(snapshot) => {
            git_patch_between_commits(&args.repo_cwd, &snapshot.base_sha, &snapshot.commit_sha)
                .with_context(|| {
                    format!(
                        "generate patch for repo snapshot {}..{}",
                        snapshot.base_sha, snapshot.commit_sha
                    )
                })?
        }
        None => git_patch_against_head(&args.repo_cwd)?,
    };
    let patch_path = bundle_dir.join("repo.patch");
    std::fs::write(&patch_path, patch)
        .with_context(|| format!("write patch {}", patch_path.display()))?;

    if args.include_logs {
        let logs_path = bundle_dir.join("codex-logs.log");
        let bytes = args
            .logs_bytes
            .clone()
            .unwrap_or_else(|| Vec::with_capacity(0));
        std::fs::write(&logs_path, bytes)
            .with_context(|| format!("write logs {}", logs_path.display()))?;
    }

    let manifest = EvalCaseManifestV0 {
        version: "v0".to_string(),
        case_id: case_id.clone(),
        created_at: created_at_rfc3339,
        conversation_id: args.conversation_id.clone(),
        source: "cli".to_string(),
        rollout: RolloutInfo {
            filename: "rollout.jsonl".to_string(),
            start: args.start.clone(),
        },
        repo: RepoInfo {
            cwd: args.repo_cwd.display().to_string(),
            git_base: GitBase {
                sha: base_sha,
                note: "For reproducibility, the base commit should be reachable (e.g. pushed / on main)."
                    .to_string(),
            },
            patch_filename: "repo.patch".to_string(),
        },
        notes: args.notes.clone(),
        artifacts: Artifacts {
            include_logs: args.include_logs,
        },
    };
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest).context("serialize manifest")?;
    std::fs::write(&manifest_path, format!("{manifest_json}\n"))
        .with_context(|| format!("write manifest {}", manifest_path.display()))?;

    Ok(CreateEvalCaseResult {
        case_id,
        path: bundle_dir,
    })
}

fn git_patch_against_head(repo_cwd: &Path) -> anyhow::Result<(String, Vec<u8>)> {
    let base_sha =
        git_stdout(repo_cwd, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());

    let mut patch = Vec::new();
    if let Ok(mut bytes) = git_diff(
        repo_cwd,
        &["diff", "--no-textconv", "--no-ext-diff", "--binary", "HEAD"],
    ) {
        patch.append(&mut bytes);
    }

    let untracked =
        git_stdout(repo_cwd, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    for file in untracked.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let args = [
            "diff",
            "--no-textconv",
            "--no-ext-diff",
            "--binary",
            "--no-index",
            "--",
            null_device,
            file,
        ];
        if let Ok(mut bytes) = git_diff(repo_cwd, &args) {
            patch.append(&mut bytes);
        }
    }

    Ok((base_sha, patch))
}

fn git_stdout(repo_cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_cwd)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!("git {} failed with {}", args.join(" "), output.status);
    }
    let out = String::from_utf8(output.stdout).context("decode git stdout")?;
    Ok(out.trim().to_string())
}

fn git_diff(repo_cwd: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_cwd)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;

    let exit_ok = output.status.code().is_some_and(|c| c == 0 || c == 1);
    if !exit_ok {
        anyhow::bail!("git {} failed with {}", args.join(" "), output.status);
    }
    Ok(output.stdout)
}

fn git_patch_between_commits(
    repo_cwd: &Path,
    base_sha: &str,
    commit_sha: &str,
) -> anyhow::Result<(String, Vec<u8>)> {
    let patch = git_diff(
        repo_cwd,
        &[
            "diff",
            "--no-textconv",
            "--no-ext-diff",
            "--binary",
            base_sha,
            commit_sha,
        ],
    )?;
    Ok((base_sha.to_string(), patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn creates_bundle_with_manifest_and_rollout() {
        let codex_home = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let repo_root = repo_dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        let init_status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(init_status.success());
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "init",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());

        std::fs::write(repo_root.join("README.md"), "changed\n").unwrap();

        let rollout_path = repo_root.join("rollout.jsonl");
        std::fs::write(&rollout_path, "line-1\nline-2\n").unwrap();

        let args = CreateEvalCaseArgs {
            codex_home: codex_home.path().to_path_buf(),
            conversation_id: "conv-1".to_string(),
            rollout_path,
            start: StartMarker {
                kind: StartMarkerKind::RolloutLineIndex,
                value: StartMarkerValue::LineIndex(1),
                display: "Start now".to_string(),
            },
            repo_cwd: repo_root.clone(),
            repo_snapshot: None,
            notes: Notes {
                what_went_wrong: "bad".to_string(),
                what_good_looks_like: "good".to_string(),
            },
            include_logs: true,
            logs_bytes: Some(b"logs".to_vec()),
        };

        let out = create_eval_case_bundle(&args).unwrap();
        assert!(!out.case_id.is_empty());
        assert!(out.path.exists());
        assert_eq!(out.path.file_name().unwrap(), out.case_id.as_str());
        assert!(out.path.starts_with(codex_home.path().join("eval-case")));
        assert!(out.case_id.contains("my-repo"));

        let manifest_text = std::fs::read_to_string(out.path.join("manifest.json")).unwrap();
        let manifest: EvalCaseManifestV0 = serde_json::from_str(&manifest_text).unwrap();
        assert_eq!(manifest.version, "v0");
        assert_eq!(manifest.conversation_id, "conv-1");
        assert_eq!(manifest.notes, args.notes);
        assert!(manifest.repo.git_base.sha != "unknown");
        assert_eq!(manifest.artifacts.include_logs, true);

        assert!(out.path.join("repo.patch").exists());
        assert!(out.path.join("rollout.jsonl").exists());
        assert_eq!(
            std::fs::read(out.path.join("codex-logs.log")).unwrap(),
            b"logs".to_vec()
        );
    }

    #[test]
    fn creates_bundle_from_repo_snapshot_commit() {
        let codex_home = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let repo_root = repo_dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_root).unwrap();

        std::fs::write(repo_root.join("README.md"), "base\n").unwrap();
        let init_status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(init_status.success());
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "base",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());

        let base_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"]).unwrap();

        // Create a snapshot commit.
        std::fs::write(repo_root.join("README.md"), "snapshot\n").unwrap();
        std::fs::write(repo_root.join("snap.txt"), "snap\n").unwrap();
        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(add_status.success());
        let commit_status = Command::new("git")
            .args([
                "-c",
                "user.name=codex",
                "-c",
                "user.email=codex@example.com",
                "commit",
                "-m",
                "snapshot",
                "-q",
            ])
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(commit_status.success());
        let snapshot_sha = git_stdout(&repo_root, &["rev-parse", "HEAD"]).unwrap();

        // Dirty the working tree after the snapshot commit; this should not affect repo.patch.
        std::fs::write(repo_root.join("README.md"), "worktree\n").unwrap();

        let rollout_path = repo_root.join("rollout.jsonl");
        std::fs::write(&rollout_path, "line-1\nline-2\n").unwrap();

        let args = CreateEvalCaseArgs {
            codex_home: codex_home.path().to_path_buf(),
            conversation_id: "conv-2".to_string(),
            rollout_path,
            start: StartMarker {
                kind: StartMarkerKind::RolloutLineIndex,
                value: StartMarkerValue::LineIndex(0),
                display: "From: test".to_string(),
            },
            repo_cwd: repo_root.clone(),
            repo_snapshot: Some(RepoSnapshot {
                base_sha: base_sha.clone(),
                commit_sha: snapshot_sha,
            }),
            notes: Notes {
                what_went_wrong: "bad".to_string(),
                what_good_looks_like: "good".to_string(),
            },
            include_logs: false,
            logs_bytes: None,
        };

        let out = create_eval_case_bundle(&args).unwrap();
        assert_eq!(out.path.file_name().unwrap(), out.case_id.as_str());
        assert!(out.path.starts_with(codex_home.path().join("eval-case")));
        assert!(out.case_id.contains("my-repo"));
        let manifest_text = std::fs::read_to_string(out.path.join("manifest.json")).unwrap();
        let manifest: EvalCaseManifestV0 = serde_json::from_str(&manifest_text).unwrap();
        assert_eq!(manifest.repo.git_base.sha, base_sha);

        let patch_text =
            String::from_utf8(std::fs::read(out.path.join("repo.patch")).unwrap()).unwrap();
        assert!(
            patch_text.contains("snapshot"),
            "patch should include snapshot commit changes"
        );
        assert!(
            !patch_text.contains("worktree"),
            "patch should not include post-snapshot working tree changes"
        );
        assert!(patch_text.contains("snap.txt"));
    }
}
