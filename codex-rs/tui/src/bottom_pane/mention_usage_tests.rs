use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn selection_frequency_persists_after_handoff_and_flush() {
    let home = tempfile::tempdir().expect("temporary home");
    let mut usage = MentionUsage::load(home.path(), HistoryPersistence::SaveAll);
    usage.record("skill://status");
    usage.record("skill://status");
    let mut replacement = MentionUsage::load(home.path(), HistoryPersistence::SaveAll);
    replacement.inherit(&mut usage);
    replacement.flush().await;

    assert_eq!(
        MentionUsage::load(home.path(), HistoryPersistence::SaveAll).frequencies(),
        HashMap::from([("skill://status".to_owned(), 2)])
    );
}

#[tokio::test]
async fn changing_storage_keeps_usage_separate_and_flushes_prior_writes() {
    let first_home = tempfile::tempdir().expect("first temporary home");
    let second_home = tempfile::tempdir().expect("second temporary home");
    let mut first = MentionUsage::load(first_home.path(), HistoryPersistence::SaveAll);
    first.record("skill://first");
    let mut second = MentionUsage::load(second_home.path(), HistoryPersistence::SaveAll);
    second.inherit(&mut first);
    second.record("skill://second");
    second.flush().await;

    assert_eq!(
        (
            MentionUsage::load(first_home.path(), HistoryPersistence::SaveAll).frequencies(),
            MentionUsage::load(second_home.path(), HistoryPersistence::SaveAll).frequencies(),
        ),
        (
            HashMap::from([("skill://first".to_owned(), 1)]),
            HashMap::from([("skill://second".to_owned(), 1)]),
        )
    );
}

#[test]
fn old_selections_expire() {
    let mut usage = MentionUsage::default();
    usage.record("skill://status");
    for _ in 0..HISTORY_LIMIT {
        usage.record("skill://other");
    }
    assert_eq!(
        usage.frequencies(),
        HashMap::from([("skill://other".to_owned(), HISTORY_LIMIT)])
    );
}

#[test]
fn selection_replaces_malformed_history() {
    let home = tempfile::tempdir().expect("temporary home");
    let path = home.path().join(FILE_NAME);
    std::fs::write(&path, "[\"truncated").expect("write malformed history");

    persist_selection(&path, "skill://status".to_owned()).expect("persist selection");

    assert_eq!(
        MentionUsage::load(home.path(), HistoryPersistence::SaveAll).frequencies(),
        HashMap::from([("skill://status".to_owned(), 1)])
    );
}

#[test]
fn contended_lock_does_not_prevent_later_selections() {
    let home = tempfile::tempdir().expect("temporary home");
    let path = home.path().join(FILE_NAME);
    let lock = File::create(path.with_extension("lock")).expect("create lock file");
    lock.lock().expect("hold lock");
    assert_eq!(
        persist_selection(&path, "skill://blocked".to_owned())
            .expect_err("lock contention")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(lock);
    persist_selection(&path, "skill://status".to_owned()).expect("persist selection");
    assert_eq!(
        MentionUsage::load(home.path(), HistoryPersistence::SaveAll).frequencies(),
        HashMap::from([("skill://status".to_owned(), 1)])
    );
}
