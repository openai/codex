//! Regression coverage for rollout snippet matching, progress, cancellation, and errors.

#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

thread_local! {
    pub(super) static REGEX_COMPILATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[tokio::test]
async fn plain_snippets_compile_once_and_match_decoded_text_across_files() -> io::Result<()> {
    let root = TempDir::new()?;
    REGEX_COMPILATIONS.set(/*value*/ 0);
    let mut matcher = RolloutSnippetMatcher::new("é\"\n[");
    let mut other = RolloutSnippetMatcher::new("unrelated");
    assert_eq!(REGEX_COMPILATIONS.get(), 0);

    for (index, message) in ["unrelated", "first É\"\n[hit]", "next é\"\n[hit]"]
        .into_iter()
        .enumerate()
    {
        let path = root.path().join(format!("{index}.jsonl"));
        let line = serde_json::json!({
            "timestamp": "2025-01-01T00:00:00Z",
            "type": "event_msg",
            "payload": { "type": "agent_message", "message": message }
        });
        std::fs::write(&path, line.to_string())?;
        let expected = (index != 0).then(|| message.replace('\n', " "));
        assert_eq!(matcher.first_match(&path).await?, expected);
        assert_eq!(REGEX_COMPILATIONS.get(), 2);
    }

    assert_eq!(
        other.first_match(&root.path().join("0.jsonl")).await?,
        Some("unrelated".to_string())
    );
    assert_eq!(REGEX_COMPILATIONS.get(), 4);
    Ok(())
}

#[tokio::test]
async fn compressed_search_preserves_error_handling() -> io::Result<()> {
    let root = TempDir::new()?;
    let line = br#"{"timestamp":"2025-01-01T00:00:00Z","type":"event_msg","payload":{"type":"agent_message","message":"needle"}}"#;
    let compressed = zstd::stream::encode_all(&line[..], 3)?;
    let mut expected = RolloutSearchMatches::new();
    for index in 0..7 {
        let path = root.path().join(format!("rollout-{index:04}.jsonl"));
        let physical_path = path.with_extension("jsonl.zst");
        std::fs::write(&physical_path, &compressed)?;
        expected.insert(path);
    }
    std::fs::write(root.path().join("rollout-broken.jsonl.zst"), b"not zstd")?;
    assert_eq!(
        scan_compressed_rollout_matches(root.path(), "needle", /*progress*/ None).await?,
        expected
    );
    let query = "a".repeat(500_000);

    assert!(case_insensitive_literal_regex(&query).is_err());
    assert_eq!(
        scan_compressed_rollout_matches(root.path(), &query, /*progress*/ None).await?,
        RolloutSearchMatches::new()
    );
    assert_eq!(
        RolloutSnippetMatcher::new(&query)
            .first_match(&root.path().join("missing.jsonl"))
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    Ok(())
}

#[tokio::test]
async fn fallback_reports_plain_and_compressed_candidates() -> io::Result<()> {
    let home = TempDir::new()?;
    let root = home.path().join(SESSIONS_SUBDIR);
    std::fs::create_dir(&root)?;
    let line = br#"{"timestamp":"2025-01-01T00:00:00Z","type":"event_msg","payload":{"type":"agent_message","message":"needle"}}"#;
    let plain = root.join("rollout-plain.jsonl");
    let compressed = root.join("rollout-compressed.jsonl");
    std::fs::write(&plain, line)?;
    std::fs::write(
        compressed.with_extension("jsonl.zst"),
        zstd::stream::encode_all(&line[..], 3)?,
    )?;
    let (sender, mut receiver) = mpsc::channel(4);
    let matches = search_rollout_matches_with_progress(
        &home.path().join("missing-rg"),
        home.path(),
        /*archived*/ false,
        "needle",
        Some(sender),
        Box::pin(pending()),
    )
    .await?;
    let mut progress = HashSet::new();
    while let Some(candidate) = receiver.recv().await {
        progress.insert(candidate.path);
    }
    assert_eq!(
        (matches, progress),
        (
            HashSet::from([plain.clone(), compressed.clone()]),
            HashSet::from([plain, compressed])
        )
    );
    Ok(())
}

#[cfg(unix)]
mod streaming {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    #[cfg(target_os = "linux")]
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    fn gated_rg(home: &Path) -> io::Result<PathBuf> {
        let root = home.join(SESSIONS_SUBDIR);
        std::fs::create_dir(&root)?;
        let rg = home.join("gated-rg");
        codex_utils_cargo_bin::write_executable(
            &rg,
            r#"#!/bin/sh
for root do :; done
printf '%s\n' "$$" > "$root/rg.pid"
printf '%s\0001\n' "$root/first
file.jsonl"
while ! test -f "$root/release"; do sleep 0.02; done
printf '%s\0001\n' "$root/second.jsonl"
"#,
        )?;
        Ok(rg)
    }

    /// A path discovered by ripgrep reaches the consumer while the child is still running.
    #[tokio::test]
    async fn reports_paths_before_ripgrep_exits() -> anyhow::Result<()> {
        let home = TempDir::new()?;
        let rg = gated_rg(home.path())?;
        let root = home.path().join(SESSIONS_SUBDIR);
        let codex_home = home.path().to_path_buf();
        let (sender, mut paths) = mpsc::channel(2);
        let search = tokio::spawn(async move {
            search_rollout_matches_with_progress(
                &rg,
                &codex_home,
                /*archived*/ false,
                "needle",
                Some(sender),
                Box::pin(pending()),
            )
            .await
        });

        assert_eq!(
            timeout(Duration::from_secs(5), paths.recv()).await?,
            Some(RolloutSearchCandidate {
                path: root.join("first\nfile.jsonl"),
            })
        );
        assert!(!search.is_finished());
        tokio::fs::write(root.join("release"), []).await?;
        let result = timeout(Duration::from_secs(5), search).await???;
        assert_eq!(
            result,
            HashSet::from([root.join("first\nfile.jsonl"), root.join("second.jsonl")])
        );
        Ok(())
    }

    /// Full progress queues never prevent ripgrep or the final search from completing.
    #[tokio::test]
    async fn full_progress_queue_does_not_stall_search() -> anyhow::Result<()> {
        let home = TempDir::new()?;
        let rg = gated_rg(home.path())?;
        let root = home.path().join(SESSIONS_SUBDIR);
        tokio::fs::write(root.join("release"), []).await?;
        let (sender, mut paths) = mpsc::channel(1);
        let matches = timeout(
            Duration::from_secs(5),
            search_rollout_matches_with_progress(
                &rg,
                home.path(),
                /*archived*/ false,
                "needle",
                Some(sender),
                Box::pin(pending()),
            ),
        )
        .await??;
        assert_eq!(
            (matches, paths.recv().await, paths.recv().await,),
            (
                HashSet::from([root.join("first\nfile.jsonl"), root.join("second.jsonl")]),
                Some(RolloutSearchCandidate {
                    path: root.join("first\nfile.jsonl"),
                }),
                None,
            )
        );
        Ok(())
    }

    /// Cancelling a held search reaps ripgrep and closes its progress stream.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancellation_reaps_ripgrep() -> anyhow::Result<()> {
        let home = TempDir::new()?;
        let rg = gated_rg(home.path())?;
        let root = home.path().join(SESSIONS_SUBDIR);
        let codex_home = home.path().to_path_buf();
        let (sender, mut paths) = mpsc::channel(2);
        let (cancel, cancelled) = oneshot::channel::<()>();
        let search = tokio::spawn(async move {
            search_rollout_matches_with_progress(
                &rg,
                &codex_home,
                /*archived*/ false,
                "needle",
                Some(sender),
                Box::pin(async {
                    let _ = cancelled.await;
                }),
            )
            .await
        });

        let candidate = timeout(Duration::from_secs(5), paths.recv()).await?;
        if candidate.is_none() {
            let outcome = timeout(Duration::from_secs(5), search).await?;
            anyhow::bail!("search ended before reporting a candidate: {outcome:?}");
        }
        assert_eq!(
            candidate,
            Some(RolloutSearchCandidate {
                path: root.join("first\nfile.jsonl"),
            })
        );
        let pid = tokio::fs::read_to_string(root.join("rg.pid")).await?;
        let child = Path::new("/proc").join(pid.trim());
        assert!(child.exists());
        cancel.send(()).expect("search is still running");
        let error = timeout(Duration::from_secs(5), search).await??.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(paths.recv().await, None);
        assert!(!child.exists());
        Ok(())
    }
}

#[cfg(unix)]
#[tokio::test]
async fn search_finds_archive_published_while_ripgrep_is_running() -> io::Result<()> {
    let home = TempDir::new()?;
    let root = home.path().join(SESSIONS_SUBDIR);
    std::fs::create_dir(&root)?;
    let line = br#"{"timestamp":"2025-01-01T00:00:00Z","type":"event_msg","payload":{"type":"agent_message","message":"needle"}}"#;
    let target = root.join("rollout-target.jsonl");
    std::fs::write(&target, line)?;
    let rg = home.path().join("rg");
    codex_utils_cargo_bin::write_executable(
        &rg,
        "#!/bin/sh\nfor root do :; done\nprintf '%s\\0001\\n' \"$root/ready\"\nread release < \"$root/release\"\nexit 1\n",
    )?;
    let fifo = root.join("release");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()?
            .success()
    );
    let (tx, mut rx) = mpsc::channel(4);
    let search = search_rollout_matches_with_progress(
        &rg,
        home.path(),
        /*archived*/ false,
        "needle",
        Some(tx),
        Box::pin(pending()),
    );
    tokio::pin!(search);
    tokio::select! {
        result = &mut search => panic!("search completed before archive publication: {result:?}"),
        candidate = rx.recv() => assert_eq!(candidate.expect("rg ready").path, root.join("ready")),
    }
    std::fs::write(
        target.with_extension("jsonl.zst"),
        zstd::stream::encode_all(&line[..], 3)?,
    )?;
    std::fs::remove_file(&target)?;
    let release = tokio::task::spawn_blocking(move || std::fs::write(fifo, b"release\n"));
    let matches = search.await?;
    release.await.map_err(io::Error::other)??;
    assert!(matches.contains(&target));
    Ok(())
}
