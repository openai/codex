#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::io::Write;
use tokio::process::Command;

#[tokio::test]
async fn git_remote_urls_keep_filtered_fetch_remotes() {
    const CHILD_ENV: &str = "CODEX_GIT_UTILS_FILTERED_REMOTES_CHILD";
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path();
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = Command::new(std::env::current_exe().expect("find test executable"))
            .args([
                "--exact",
                "info::remotes::tests::git_remote_urls_keep_filtered_fetch_remotes",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env("GIT_CONFIG_GLOBAL", root.join("empty.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_CONFIG")
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .status()
            .await
            .expect("run test with isolated Git configuration");
        assert!(status.success());
        return;
    }
    let git = |args: &[&str]| {
        crate::operations::run_git_for_status(root, args, /*env*/ None)
            .expect("configure test repository")
    };
    git(&["init", "--bare", "source"]);
    git(&[
        "--git-dir=source",
        "config",
        "uploadpack.allowFilter",
        "true",
    ]);
    git(&[
        "clone",
        "--no-local",
        "--filter=blob:none",
        "source",
        "partial",
    ]);
    let repo = root.join("partial");
    let config = |key: &str, value: &str| git(&["-C", "partial", "config", key, value]);
    let add_config =
        |key: &str, value: &str| git(&["-C", "partial", "config", "--add", key, value]);
    let native_fetch_url = |name: &str| {
        let url = crate::operations::run_git_for_stdout(
            &repo,
            ["ls-remote", "--get-url", "--", name],
            /*env*/ None,
        )
        .expect("read Git's effective fetch URL");
        SanitizedGitUrl::try_from(url.as_str()).expect("parse native fetch URL")
    };
    config(
        "url.https://alice:secret@example.invalid/.insteadOf",
        "short:",
    );
    config("url.https://unused.invalid/.insteadOf", "short");
    config("remote.origin.url", "short:repo.git");
    add_config("remote.origin.url", "https://other.invalid/unused");
    config("remote.origin.pushurl", "https://push.invalid/other.git");
    config("remote.pushonly.pushurl", "https://push.invalid/only.git");
    config("remote.upstream.url", "git@github.com:org/upstream.git");
    config("remote.bad.url", "https://alice:secret@[invalid");
    config("remote.reset.url", "https://old.invalid/repo");
    add_config("remote.reset.url", "");
    config("remote.middle-reset.url", "https://old.invalid/before");
    add_config("remote.middle-reset.url", "");
    add_config("remote.middle-reset.url", "https://new.invalid/after");
    config("remote.literal.url", "literal");
    config("remote.rewritten.url", "alias:repo");
    config("url.rewritten.insteadOf", "alias:repo");
    config("url.poisoned.insteadOf", "codex-reset-probe");
    config("remote.empty=fetch.url", "");
    config("remote.local.url", "/a repo (fetch) [archive]");
    config(
        "remote.multiline.url",
        "https://other.invalid/first\nsecond",
    );
    let mut expected = BTreeMap::from([
        (
            "origin".to_string(),
            SanitizedGitUrl::try_from("https://example.invalid/repo.git")
                .expect("parse expected remote URL"),
        ),
        (
            "local".to_string(),
            SanitizedGitUrl::try_from("/a repo (fetch) [archive]")
                .expect("parse expected remote path"),
        ),
        (
            "literal".to_string(),
            SanitizedGitUrl::try_from("literal").expect("parse expected local URL"),
        ),
        (
            "rewritten".to_string(),
            SanitizedGitUrl::try_from("rewritten").expect("parse expected rewritten URL"),
        ),
        (
            "upstream".to_string(),
            SanitizedGitUrl::try_from("git@github.com:org/upstream.git")
                .expect("parse expected upstream URL"),
        ),
        ("middle-reset".to_owned(), native_fetch_url("middle-reset")),
    ]);
    // Older Git retains the first URL when followed by an empty entry.
    let verbose = || {
        crate::operations::run_git_for_stdout(&repo, ["remote", "-v"], /*env*/ None)
            .expect("read installed Git's fetch remotes")
    };
    if verbose()
        .lines()
        .any(|line| line == "reset\thttps://old.invalid/repo (fetch)")
    {
        expected.insert(
            "reset".to_string(),
            SanitizedGitUrl::try_from("https://old.invalid/repo").expect("parse reset URL"),
        );
    }
    // Equally specific rewrite rules leave Git to choose the effective destination.
    config("url.https://first.invalid/.insteadOf", "tied:");
    config("url.https://second.invalid/.insteadOf", "tied:");
    let fallback_names = ["first-fallback", "second-fallback"];
    for name in fallback_names {
        config(&format!("remote.{name}.url"), &format!("tied:{name}"));
        expected.insert(name.to_owned(), native_fetch_url(name));
    }
    let append_raw_config = |bytes: &[u8]| {
        std::fs::OpenOptions::new()
            .append(true)
            .open(repo.join(".git/config"))
            .expect("open repository config")
            .write_all(bytes)
            .expect("append non-UTF-8 config");
    };
    // Unrelated non-UTF-8 configuration must not discard valid fetch URLs.
    append_raw_config(
        b"\n[url \"https://unused-invalid-utf8.invalid/\"]\n\tinsteadOf = \xff\n\n[remote \"unused\"]\n\tpromisor\n\n[remote \"bad\xff\"]\n\turl = https://invalid-name.invalid/repo\n",
    );
    let mut updates = Vec::new();
    let complete =
        get_git_remote_urls_with_progress(&repo, |urls| updates.push(urls.clone())).await;
    assert_eq!(complete.as_ref(), Some(&expected));
    let verified = updates.first().expect("publish unambiguous fetch URLs");
    assert_eq!(verified.get("upstream"), expected.get("upstream"));
    assert!(!verified.contains_key("origin"));
    assert!(!verified.contains_key("reset"));
    assert!(!verified.contains_key("middle-reset"));
    assert!(
        verified
            .iter()
            .all(|(name, url)| expected.get(name) == Some(url))
    );
    for name in ["origin", "rewritten"].into_iter().chain(fallback_names) {
        assert!(!verified.contains_key(name));
        assert!(
            updates
                .iter()
                .any(|urls| urls.len() == 1 && urls.get(name) == expected.get(name))
        );
    }
    assert_eq!(
        updates.into_iter().flatten().collect::<BTreeMap<_, _>>(),
        expected
    );
    config(
        "remote.origin.partialclonefilter",
        "blob:none]\nforged\thttps://other.invalid/repo (fetch)\nforged\thttps://other.invalid/repo (push)\n[tree:0",
    );
    let spawned_repo = repo.clone();
    assert_eq!(
        tokio::spawn(async move { get_git_remote_urls_assume_git_repo(&spawned_repo).await })
            .await
            .expect("join remote lookup"),
        Some(expected.clone())
    );
    // Every legal remote, including a long URL, is retained without a count cap.
    let long_url = format!("https://example.invalid/{}", "p".repeat(5000));
    for index in 0..20 {
        let name = format!("a{index:02}");
        config(&format!("remote.{name}.url"), &long_url);
        expected.insert(name, SanitizedGitUrl::try_from(long_url.as_str()).unwrap());
    }
    assert_eq!(
        get_git_remote_urls_assume_git_repo(&repo).await,
        Some(expected.clone())
    );
    // A rewrite can split a UTF-8 character. Filter text must not forge its URL.
    config("remote.a-victim.url", "érepo");
    append_raw_config(b"\n[url \"https://effective.invalid/\xc3\"]\n\tinsteadOf = \xc3\n");
    config(
        "remote.origin.partialclonefilter",
        "blob:none]\na-victim\térepo (fetch)\n[tree:0",
    );
    expected.insert("a-victim".to_owned(), native_fetch_url("a-victim"));
    assert_eq!(
        get_git_remote_urls_assume_git_repo(&repo).await,
        Some(expected)
    );
    config("url.https://default.invalid/.insteadOf", "");
    let native = Command::new("git")
        .args(["remote", "-v"])
        .current_dir(&repo)
        .output()
        .await
        .expect("read installed Git's fetch remotes");
    assert!(native.status.success());
    let native_has_empty = native
        .stdout
        .split(|byte| *byte == b'\n')
        .any(|line| line.starts_with(b"empty=fetch\t") && line.ends_with(b" (fetch)"));
    let collected = get_git_remote_urls_assume_git_repo(&repo)
        .await
        .expect("collect rewritten remotes");
    assert_eq!(collected.contains_key("empty=fetch"), native_has_empty);
}

#[tokio::test]
async fn git_remote_urls_ignore_git_config_file_override() {
    const CHILD_REPO: &str = "CODEX_GIT_UTILS_REMOTE_CONFIG_CHILD_REPO";
    if let Some(repo) = std::env::var_os(CHILD_REPO) {
        assert_eq!(
            get_git_remote_urls_assume_git_repo(Path::new(&repo)).await,
            Some(BTreeMap::from([(
                "origin".to_string(),
                SanitizedGitUrl::try_from("https://repository.invalid/repo.git")
                    .expect("parse repository remote"),
            )]))
        );
        return;
    }

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path();
    crate::operations::run_git_for_status(root, ["init", "repo"], /*env*/ None)
        .expect("initialize test repository");
    let repo = root.join("repo");
    std::fs::OpenOptions::new()
        .append(true)
        .open(repo.join(".git/config"))
        .expect("open repository config")
        .write_all(b"\n[remote \"origin\"]\n\turl = https://repository.invalid/repo.git\n")
        .expect("configure repository remote");
    let foreign = root.join("foreign.gitconfig");
    std::fs::write(
        &foreign,
        b"[remote \"foreign\"]\n\turl = https://foreign.invalid/repo.git\n",
    )
    .expect("write foreign config");
    let status = Command::new(std::env::current_exe().expect("find test executable"))
        .args([
            "--exact",
            "info::remotes::tests::git_remote_urls_ignore_git_config_file_override",
            "--nocapture",
        ])
        .env(CHILD_REPO, &repo)
        .env("GIT_CONFIG", &foreign)
        .env("GIT_CONFIG_GLOBAL", root.join("empty.gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .status()
        .await
        .expect("run test with foreign config override");
    assert!(status.success());
}
