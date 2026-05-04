use super::*;
use crate::config::ConfigBuilder;
use codex_exec_server::LOCAL_FS;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::PathBufExt;
use core_test_support::TempDirExt;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

async fn get_user_instructions(config: &Config) -> Option<String> {
    AgentsMdManager::new(config)
        .user_instructions_with_fs(LOCAL_FS.as_ref())
        .await
}

async fn agents_md_paths(config: &Config) -> std::io::Result<Vec<AbsolutePathBuf>> {
    AgentsMdManager::new(config)
        .agents_md_paths(LOCAL_FS.as_ref())
        .await
}

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

    config.cwd = root.abs();
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

async fn make_config_with_project_root_markers(
    root: &TempDir,
    limit: usize,
    instructions: Option<&str>,
    markers: &[&str],
) -> Config {
    let codex_home = TempDir::new().unwrap();
    let cli_overrides = vec![(
        "project_root_markers".to_string(),
        TomlValue::Array(
            markers
                .iter()
                .map(|marker| TomlValue::String((*marker).to_string()))
                .collect(),
        ),
    )];
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(cli_overrides)
        .build()
        .await
        .expect("defaults for test should always succeed");

    config.cwd = root.abs();
    config.project_doc_max_bytes = limit;
    config.user_instructions = instructions.map(ToOwned::to_owned);
    config
}

/// AGENTS.md missing – should yield `None`.
#[tokio::test]
async fn no_doc_file_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await)
            .await;
    assert!(
        res.is_none(),
        "Expected None when AGENTS.md is absent and no system instructions provided"
    );
    assert!(res.is_none(), "Expected None when AGENTS.md is absent");
}

#[tokio::test]
async fn no_environment_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config = make_config(&tmp, /*limit*/ 4096, Some("user instructions")).await;

    let res = AgentsMdManager::new(&config)
        .user_instructions(/*environment*/ None)
        .await;

    assert_eq!(res, None);
}

/// Small file within the byte-limit is returned unmodified.
#[tokio::test]
async fn doc_smaller_than_limit_is_returned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "hello world").unwrap();

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await)
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

    let res = get_user_instructions(&make_config(&tmp, LIMIT, /*instructions*/ None).await)
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
    let mut cfg = make_config(&repo, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.cwd = nested.abs();

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "root level doc");
}

/// Explicitly setting the byte-limit to zero disables project docs.
#[tokio::test]
async fn zero_byte_limit_disables_docs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 0, /*instructions*/ None).await).await;
    assert!(
        res.is_none(),
        "With limit 0 the function should return None"
    );
}

#[tokio::test]
async fn zero_byte_limit_disables_discovery() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

    let discovery = agents_md_paths(&make_config(&tmp, /*limit*/ 0, /*instructions*/ None).await)
        .await
        .expect("discover paths");
    assert_eq!(discovery, Vec::<AbsolutePathBuf>::new());
}

/// When both system instructions and AGENTS.md docs are present the two
/// should be concatenated with the separator.
#[tokio::test]
async fn merges_existing_instructions_with_agents_md() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "proj doc").unwrap();

    const INSTRUCTIONS: &str = "base instructions";

    let res = get_user_instructions(&make_config(&tmp, /*limit*/ 4096, Some(INSTRUCTIONS)).await)
        .await
        .expect("should produce a combined instruction string");

    let expected = format!("{INSTRUCTIONS}{AGENTS_MD_SEPARATOR}{}", "proj doc");

    assert_eq!(res, expected);
}

/// If there are existing system instructions but AGENTS.md docs are
/// missing we expect the original instructions to be returned unchanged.
#[tokio::test]
async fn keeps_existing_instructions_when_doc_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");

    const INSTRUCTIONS: &str = "some instructions";

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, Some(INSTRUCTIONS)).await).await;

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

    let mut cfg = make_config(&repo, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.cwd = nested.abs();

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "root doc\n\ncrate doc");
}

#[tokio::test]
async fn project_root_markers_are_honored_for_agents_discovery() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::write(root.path().join(".codex-root"), "").unwrap();
    fs::write(root.path().join("AGENTS.md"), "parent doc").unwrap();

    let nested = root.path().join("dir1");
    fs::create_dir_all(nested.join(".git")).unwrap();
    fs::write(nested.join("AGENTS.md"), "child doc").unwrap();

    let mut cfg = make_config_with_project_root_markers(
        &root,
        /*limit*/ 4096,
        /*instructions*/ None,
        &[".codex-root"],
    )
    .await;
    cfg.cwd = nested.abs();

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    let expected_parent = AbsolutePathBuf::try_from(
        dunce::canonicalize(root.path().join("AGENTS.md")).expect("canonical parent doc path"),
    )
    .expect("absolute parent doc path");
    let expected_child = AbsolutePathBuf::try_from(
        dunce::canonicalize(cfg.cwd.join("AGENTS.md")).expect("canonical child doc path"),
    )
    .expect("absolute child doc path");
    assert_eq!(discovery.len(), 2);
    assert_eq!(discovery[0], expected_parent);
    assert_eq!(discovery[1], expected_child);

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "parent doc\n\nchild doc");
}

#[tokio::test]
async fn instruction_sources_include_global_before_agents_md_docs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "project doc").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, Some("global doc")).await;
    let global_agents = cfg.codex_home.join(DEFAULT_AGENTS_MD_FILENAME);
    fs::create_dir_all(&cfg.codex_home).unwrap();
    fs::write(&global_agents, "global doc").unwrap();

    let sources = AgentsMdManager::new(&cfg)
        .instruction_sources(LOCAL_FS.as_ref())
        .await;
    let project_agents = AbsolutePathBuf::try_from(
        dunce::canonicalize(cfg.cwd.join("AGENTS.md")).expect("canonical project doc path"),
    )
    .expect("absolute project doc path");

    assert_eq!(sources, vec![global_agents, project_agents]);
}

/// AGENTS.override.md is preferred over AGENTS.md when both are present.
#[tokio::test]
async fn agents_local_md_preferred() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DEFAULT_AGENTS_MD_FILENAME), "versioned").unwrap();
    fs::write(tmp.path().join(LOCAL_AGENTS_MD_FILENAME), "local").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("local doc expected");

    assert_eq!(res, "local");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert_eq!(
        discovery[0].file_name().unwrap().to_string_lossy(),
        LOCAL_AGENTS_MD_FILENAME
    );
}

/// When AGENTS.md is absent but a configured fallback exists, the fallback is used.
#[tokio::test]
async fn uses_configured_fallback_when_agents_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("EXAMPLE.md"), "example instructions").unwrap();

    let cfg = make_config_with_fallback(
        &tmp,
        /*limit*/ 4096,
        /*instructions*/ None,
        &["EXAMPLE.md"],
    )
    .await;

    let res = get_user_instructions(&cfg)
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

    let cfg = make_config_with_fallback(
        &tmp,
        /*limit*/ 4096,
        /*instructions*/ None,
        &["EXAMPLE.md", ".example.md"],
    )
    .await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("AGENTS.md should win");

    assert_eq!(res, "primary");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert!(
        discovery[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq(DEFAULT_AGENTS_MD_FILENAME)
    );
}

#[tokio::test]
async fn agents_md_directory_is_ignored() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join("AGENTS.md")).unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery, Vec::<AbsolutePathBuf>::new());
}

#[cfg(unix)]
#[tokio::test]
async fn agents_md_special_file_is_ignored() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("AGENTS.md");
    let c_path = CString::new(path.as_os_str().as_bytes()).expect("path without nul");
    // SAFETY: `c_path` is a valid, nul-terminated path and `mkfifo` does not
    // retain the pointer after the call.
    let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
    assert_eq!(rc, 0);

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery, Vec::<AbsolutePathBuf>::new());
}

#[tokio::test]
async fn override_directory_falls_back_to_agents_md_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join(LOCAL_AGENTS_MD_FILENAME)).unwrap();
    fs::write(tmp.path().join(DEFAULT_AGENTS_MD_FILENAME), "primary").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("AGENTS.md should be used when override is a directory");
    assert_eq!(res, "primary");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert_eq!(
        discovery[0]
            .file_name()
            .expect("file name")
            .to_string_lossy(),
        DEFAULT_AGENTS_MD_FILENAME
    );
}

#[tokio::test]
async fn skills_are_not_appended_to_agents_md() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    create_skill(
        cfg.codex_home.to_path_buf(),
        "pdf-processing",
        "extract from pdfs",
    );

    let res = get_user_instructions(&cfg)
        .await
        .expect("instructions expected");
    assert_eq!(res, "base doc");
}

#[tokio::test]
async fn apps_feature_does_not_emit_user_instructions_by_itself() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);
}

#[tokio::test]
async fn apps_feature_does_not_append_to_agents_md_user_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

    let mut cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = get_user_instructions(&cfg)
        .await
        .expect("instructions expected");
    assert_eq!(res, "base doc");
}

fn create_skill(codex_home: PathBuf, name: &str, description: &str) {
    let skill_dir = codex_home.join(format!("skills/{name}"));
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}
