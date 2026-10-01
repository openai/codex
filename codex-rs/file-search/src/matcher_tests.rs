//! Regression coverage for associating asynchronous matcher results with their query.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn query_update_does_not_relabel_pending_matches() {
    let root = tempfile::tempdir().unwrap();
    let (work_tx, work_rx) = unbounded();
    let notify_tx = work_tx.clone();
    let mut nucleo = Nucleo::new(
        Config::DEFAULT.match_paths(),
        Arc::new(move || {
            let _ = notify_tx.send(WorkSignal::NucleoNotify);
        }),
        /*num_threads*/ Some(1),
        /*columns*/ 1,
    );
    let injector = nucleo.injector();
    for (relative, match_type) in [("", MatchType::Directory), ("beta.txt", MatchType::File)] {
        let full_path = root.path().join(relative);
        injector.push(
            IndexedEntry {
                full_path: Arc::from(full_path.to_str().unwrap()),
                match_type,
            },
            |_, columns| columns[0] = Utf32String::from(relative),
        );
    }
    while nucleo.tick(/*timeout*/ 10).running {
        work_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let inner = SessionInner {
        search_directories: vec![root.path().to_path_buf()],
        limit: 50,
        threads: 1,
        compute_indices: true,
        respect_gitignore: true,
        cancelled: Arc::new(AtomicBool::new(false)),
        shutdown: Arc::new(AtomicBool::new(false)),
        reporter: Arc::new(tests::RecordingReporter::default()),
        work_tx,
    };
    let mut indices_matcher = Some(Matcher::new(Config::DEFAULT.match_paths()));
    let initial = snapshot::for_query(
        &inner,
        &nucleo,
        "",
        &mut indices_matcher,
        /*walk_complete*/ true,
    )
    .unwrap();
    assert_eq!(initial.matches.len(), 2);

    nucleo.pattern.reparse(
        /*column*/ 0,
        "bet",
        CaseMatching::Ignore,
        Normalization::Smart,
        /*append*/ true,
    );
    // The requested pattern has changed, but no tick has replaced the previous
    // snapshot. This is the same intermediate state a busy tick can expose.
    assert_eq!(
        snapshot::for_query(
            &inner,
            &nucleo,
            "bet",
            &mut indices_matcher,
            /*walk_complete*/ true
        ),
        None
    );

    while nucleo.tick(/*timeout*/ 10).running {
        work_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert_eq!(
        snapshot::for_query(
            &inner,
            &nucleo,
            "bet",
            &mut indices_matcher,
            /*walk_complete*/ true
        ),
        Some(FileSearchSnapshot {
            query: "bet".to_string(),
            matches: vec![FileMatch {
                score: 84,
                path: PathBuf::from("beta.txt"),
                match_type: MatchType::File,
                root: root.path().to_path_buf(),
                indices: Some(vec![0, 1, 2]),
            }],
            total_match_count: 1,
            scanned_file_count: 2,
            walk_complete: true,
        }),
    );
}
