//! Project-level documentation discovery.
//!
//! Project-level documentation is primarily stored in files named `AGENTS.md`.
//! Additional fallback filenames can be configured via `project_doc_fallback_filenames`.
//! We include the concatenation of all files found along the path from the
//! repository root to the current working directory as follows:
//!
//! 1.  Determine the Git repository root by walking upwards from the current
//!     working directory until a `.git` directory or file is found. If no Git
//!     root is found, only the current working directory is considered.
//! 2.  Collect every `AGENTS.md` found from the repository root down to the
//!     current working directory (inclusive) and concatenate their contents in
//!     that order.
//! 3.  We do **not** walk past the Git root.

use crate::config::Config;
use crate::features::Feature;
use crate::skills::SkillMetadata;
use crate::skills::render_skills_section;
use dunce::canonicalize as normalize_path;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tracing::error;

pub(crate) const HIERARCHICAL_AGENTS_MESSAGE: &str =
    include_str!("../hierarchical_agents_message.md");

/// Default filename scanned for project-level docs.
pub const DEFAULT_PROJECT_DOC_FILENAME: &str = "AGENTS.md";
/// Preferred local override for project-level docs.
pub const LOCAL_PROJECT_DOC_FILENAME: &str = "AGENTS.override.md";

/// When both `Config::instructions` and the project doc are present, they will
/// be concatenated with the following separator.
const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Combines `Config::instructions` and `AGENTS.md` (if present) into a single
/// string of instructions.
pub(crate) async fn get_user_instructions(
    config: &Config,
    skills: Option<&[SkillMetadata]>,
) -> Option<String> {
    let project_docs = read_project_docs(config).await;

    let mut output = String::new();

    if let Some(instructions) = config.user_instructions.clone() {
        output.push_str(&instructions);
    }

    match project_docs {
        Ok(Some(docs)) => {
            if !output.is_empty() {
                output.push_str(PROJECT_DOC_SEPARATOR);
            }
            output.push_str(&docs);
        }
        Ok(None) => {}
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
        }
    };

    let skills_section = skills.and_then(render_skills_section);
    if let Some(skills_section) = skills_section {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&skills_section);
    }

    if config.features.enabled(Feature::ChildAgentsMd) {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(HIERARCHICAL_AGENTS_MESSAGE);
    }

    if !output.is_empty() {
        Some(output)
    } else {
        None
    }
}

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))` where `contents` is the
/// concatenation of all discovered docs. If no documentation file is found the
/// function returns `Ok(None)`. Unexpected I/O failures bubble up as `Err` so
/// callers can decide how to handle them.
pub async fn read_project_docs(config: &Config) -> std::io::Result<Option<String>> {
    let max_total = config.project_doc_max_bytes;

    if max_total == 0 {
        return Ok(None);
    }

    let paths = discover_project_doc_paths(config)?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut remaining: u64 = max_total as u64;
    let mut parts: Vec<String> = Vec::new();

    for p in paths {
        if remaining == 0 {
            break;
        }

        let file = match tokio::fs::File::open(&p).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };

        let size = file.metadata().await?.len();
        let mut reader = tokio::io::BufReader::new(file).take(remaining);
        let mut data: Vec<u8> = Vec::new();
        reader.read_to_end(&mut data).await?;

        if size > remaining {
            tracing::warn!(
                "Project doc `{}` exceeds remaining budget ({} bytes) - truncating.",
                p.display(),
                remaining,
            );
        }

        let text = String::from_utf8_lossy(&data).to_string();
        if !text.trim().is_empty() {
            parts.push(text);
            remaining = remaining.saturating_sub(data.len() as u64);
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Discover the list of AGENTS.md files using the same search rules as
/// `read_project_docs`, but return the file paths instead of concatenated
/// contents. The list is ordered from repository root to the current working
/// directory (inclusive). Symlinks are allowed. When `project_doc_max_bytes`
/// is zero, returns an empty list.
pub fn discover_project_doc_paths(config: &Config) -> std::io::Result<Vec<PathBuf>> {
    let mut dir = config.cwd.clone();
    if let Ok(canon) = normalize_path(&dir) {
        dir = canon;
    }

    // Build chain from cwd upwards and detect git root.
    let mut chain: Vec<PathBuf> = vec![dir.clone()];
    let mut git_root: Option<PathBuf> = None;
    let mut cursor = dir;
    while let Some(parent) = cursor.parent() {
        let git_marker = cursor.join(".git");
        let git_exists = match std::fs::metadata(&git_marker) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e),
        };

        if git_exists {
            git_root = Some(cursor.clone());
            break;
        }

        chain.push(parent.to_path_buf());
        cursor = parent.to_path_buf();
    }

    let search_dirs: Vec<PathBuf> = if let Some(root) = git_root {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut saw_root = false;
        for p in chain.iter().rev() {
            if !saw_root {
                if p == &root {
                    saw_root = true;
                } else {
                    continue;
                }
            }
            dirs.push(p.clone());
        }
        dirs
    } else {
        vec![config.cwd.clone()]
    };

    let mut found: Vec<PathBuf> = Vec::new();
    let candidate_filenames = candidate_filenames(config);
    for d in search_dirs {
        for name in &candidate_filenames {
            let candidate = d.join(name);
            match std::fs::symlink_metadata(&candidate) {
                Ok(md) => {
                    let ft = md.file_type();
                    // Allow regular files and symlinks; opening will later fail for dangling links.
                    if ft.is_file() || ft.is_symlink() {
                        found.push(candidate);
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            }
        }
    }

    Ok(found)
}

fn candidate_filenames<'a>(config: &'a Config) -> Vec<&'a str> {
    let mut names: Vec<&'a str> =
        Vec::with_capacity(2 + config.project_doc_fallback_filenames.len());
    names.push(LOCAL_PROJECT_DOC_FILENAME);
    names.push(DEFAULT_PROJECT_DOC_FILENAME);
    for candidate in &config.project_doc_fallback_filenames {
        let candidate = candidate.as_str();
        if candidate.is_empty() {
            continue;
        }
        if !names.contains(&candidate) {
            names.push(candidate);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::skills::load_skills;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Helper that returns a `Config` pointing at `root` and using `limit` as
    /// the maximum number of bytes to embed from AGENTS.md. The caller can
    /// optionally specify a custom `instructions` string – when `None` the
    /// value is cleared to mimic a scenario where no system instructions have
    /// been configured.
    async fn make_config(root: &TempDir, limit: usize, instructions: Option<&str>) -> Config {
        let codex_home = TempDir::new().unwrap();
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("defaults for test should always succeed");

        config.cwd = root.path().to_path_buf();
        config.project_doc_max_bytes = limit;

        config.user_instructions = instructions.map(ToOwned::to_owned);
        config
    }

    async fn make_config_with_fallback(
        root: &TempDir,
        limit: usize,
        instructions: Option<&str>,
        fallbacks: &[&str],
    ) -> Config {
        let mut config = make_config(root, limit, instructions).await;
        config.project_doc_fallback_filenames = fallbacks
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        config
    }

    /// AGENTS.md missing – should yield `None`.
    #[tokio::test]
    async fn no_doc_file_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let res = get_user_instructions(&make_config(&tmp, 4096, None).await, None).await;
        assert!(
            res.is_none(),
            "Expected None when AGENTS.md is absent and no system instructions provided"
        );
        assert!(res.is_none(), "Expected None when AGENTS.md is absent");
    }

    /// Small file within the byte-limit is returned unmodified.
    #[tokio::test]
    async fn doc_smaller_than_limit_is_returned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "hello world").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 4096, None).await, None)
            .await
            .expect("doc expected");

        assert_eq!(
            res, "hello world",
            "The document should be returned verbatim when it is smaller than the limit and there are no existing instructions"
        );
    }

    /// Oversize file is truncated to `project_doc_max_bytes`.
    #[tokio::test]
    async fn doc_larger_than_limit_is_truncated() {
        const LIMIT: usize = 1024;
        let tmp = tempfile::tempdir().expect("tempdir");

        let huge = "A".repeat(LIMIT * 2); // 2 KiB
        fs::write(tmp.path().join("AGENTS.md"), &huge).unwrap();

        let res = get_user_instructions(&make_config(&tmp, LIMIT, None).await, None)
            .await
            .expect("doc expected");

        assert_eq!(res.len(), LIMIT, "doc should be truncated to LIMIT bytes");
        assert_eq!(res, huge[..LIMIT]);
    }

    /// When `cwd` is nested inside a repo, the search should locate AGENTS.md
    /// placed at the repository root (identified by `.git`).
    #[tokio::test]
    async fn finds_doc_in_repo_root() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository. Note .git can be a file or a directory.
        std::fs::write(
            repo.path().join(".git"),
            "gitdir: /path/to/actual/git/dir\n",
        )
        .unwrap();

        // Put the doc at the repo root.
        fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();

        // Now create a nested working directory: repo/workspace/crate_a
        let nested = repo.path().join("workspace/crate_a");
        std::fs::create_dir_all(&nested).unwrap();

        // Build config pointing at the nested dir.
        let mut cfg = make_config(&repo, 4096, None).await;
        cfg.cwd = nested;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("doc expected");
        assert_eq!(res, "root level doc");
    }

    /// Explicitly setting the byte-limit to zero disables project docs.
    #[tokio::test]
    async fn zero_byte_limit_disables_docs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 0, None).await, None).await;
        assert!(
            res.is_none(),
            "With limit 0 the function should return None"
        );
    }

    /// When both system instructions *and* a project doc are present the two
    /// should be concatenated with the separator.
    #[tokio::test]
    async fn merges_existing_instructions_with_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "proj doc").unwrap();

        const INSTRUCTIONS: &str = "base instructions";

        let res = get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await, None)
            .await
            .expect("should produce a combined instruction string");

        let expected = format!("{INSTRUCTIONS}{PROJECT_DOC_SEPARATOR}{}", "proj doc");

        assert_eq!(res, expected);
    }

    /// If there are existing system instructions but the project doc is
    /// missing we expect the original instructions to be returned unchanged.
    #[tokio::test]
    async fn keeps_existing_instructions_when_doc_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");

        const INSTRUCTIONS: &str = "some instructions";

        let res =
            get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)).await, None).await;

        assert_eq!(res, Some(INSTRUCTIONS.to_string()));
    }

    /// When both the repository root and the working directory contain
    /// AGENTS.md files, their contents are concatenated from root to cwd.
    #[tokio::test]
    async fn concatenates_root_and_cwd_docs() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository.
        std::fs::write(
            repo.path().join(".git"),
            "gitdir: /path/to/actual/git/dir\n",
        )
        .unwrap();

        // Repo root doc.
        fs::write(repo.path().join("AGENTS.md"), "root doc").unwrap();

        // Nested working directory with its own doc.
        let nested = repo.path().join("workspace/crate_a");
        std::fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("AGENTS.md"), "crate doc").unwrap();

        let mut cfg = make_config(&repo, 4096, None).await;
        cfg.cwd = nested;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("doc expected");
        assert_eq!(res, "root doc\n\ncrate doc");
    }

    /// AGENTS.override.md is preferred over AGENTS.md when both are present.
    #[tokio::test]
    async fn agents_local_md_preferred() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join(DEFAULT_PROJECT_DOC_FILENAME), "versioned").unwrap();
        fs::write(tmp.path().join(LOCAL_PROJECT_DOC_FILENAME), "local").unwrap();

        let cfg = make_config(&tmp, 4096, None).await;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("local doc expected");

        assert_eq!(res, "local");

        let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
        assert_eq!(discovery.len(), 1);
        assert_eq!(
            discovery[0].file_name().unwrap().to_string_lossy(),
            LOCAL_PROJECT_DOC_FILENAME
        );
    }

    /// When AGENTS.md is absent but a configured fallback exists, the fallback is used.
    #[tokio::test]
    async fn uses_configured_fallback_when_agents_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("EXAMPLE.md"), "example instructions").unwrap();

        let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md"]).await;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("fallback doc expected");

        assert_eq!(res, "example instructions");
    }

    /// AGENTS.md remains preferred when both AGENTS.md and fallbacks are present.
    #[tokio::test]
    async fn agents_md_preferred_over_fallbacks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "primary").unwrap();
        fs::write(tmp.path().join("EXAMPLE.md"), "secondary").unwrap();

        let cfg = make_config_with_fallback(&tmp, 4096, None, &["EXAMPLE.md", ".example.md"]).await;

        let res = get_user_instructions(&cfg, None)
            .await
            .expect("AGENTS.md should win");

        assert_eq!(res, "primary");

        let discovery = discover_project_doc_paths(&cfg).expect("discover paths");
        assert_eq!(discovery.len(), 1);
        assert!(
            discovery[0]
                .file_name()
                .unwrap()
                .to_string_lossy()
                .eq(DEFAULT_PROJECT_DOC_FILENAME)
        );
    }

    #[tokio::test]
    async fn skills_are_appended_to_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

        let cfg = make_config(&tmp, 4096, None).await;
        create_skill(
            cfg.codex_home.clone(),
            "pdf-processing",
            "extract from pdfs",
        );

        let skills = load_skills(&cfg);
        let res = get_user_instructions(
            &cfg,
            skills.errors.is_empty().then_some(skills.skills.as_slice()),
        )
        .await
        .expect("instructions expected");
        let expected_path = dunce::canonicalize(
            cfg.codex_home
                .join("skills/pdf-processing/SKILL.md")
                .as_path(),
        )
        .unwrap_or_else(|_| cfg.codex_home.join("skills/pdf-processing/SKILL.md"));
        let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
        let usage_rules = "- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
        let expected = format!(
            "base doc\n\n## Skills\nA skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.\n### Available skills\n- pdf-processing: extract from pdfs (file: {expected_path_str})\n### How to use skills\n{usage_rules}"
        );
        assert_eq!(res, expected);
    }

    #[tokio::test]
    async fn skills_render_without_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = make_config(&tmp, 4096, None).await;
        create_skill(cfg.codex_home.clone(), "linting", "run clippy");

        let skills = load_skills(&cfg);
        let res = get_user_instructions(
            &cfg,
            skills.errors.is_empty().then_some(skills.skills.as_slice()),
        )
        .await
        .expect("instructions expected");
        let expected_path =
            dunce::canonicalize(cfg.codex_home.join("skills/linting/SKILL.md").as_path())
                .unwrap_or_else(|_| cfg.codex_home.join("skills/linting/SKILL.md"));
        let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
        let usage_rules = "- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.\n  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.\n  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.\n  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.\n  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";
        let expected = format!(
            "## Skills\nA skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.\n### Available skills\n- linting: run clippy (file: {expected_path_str})\n### How to use skills\n{usage_rules}"
        );
        assert_eq!(res, expected);
    }

    fn create_skill(codex_home: PathBuf, name: &str, description: &str) {
        let skill_dir = codex_home.join(format!("skills/{name}"));
        fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }
}
