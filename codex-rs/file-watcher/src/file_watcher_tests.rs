use super::*;
use notify::event::AccessKind;
use notify::event::AccessMode;
use notify::event::CreateKind;
use notify::event::ModifyKind;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

const TEST_THROTTLE_INTERVAL: Duration = Duration::from_millis(50);

fn path(name: &str) -> PathBuf {
    PathBuf::from(name)
}

fn notify_event(kind: EventKind, paths: Vec<PathBuf>) -> Event {
    let mut event = Event::new(kind);
    for path in paths {
        event = event.add_path(path);
    }
    event
}

#[tokio::test]
async fn throttled_receiver_coalesces_within_interval() {
    let (tx, rx) = watch_channel();
    let mut throttled = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    tx.add_changed_paths(&[path("a")]).await;
    let first = timeout(Duration::from_secs(1), throttled.recv())
        .await
        .expect("first emit timeout");
    assert_eq!(
        first,
        Some(FileWatcherEvent {
            paths: vec![path("a")],
        })
    );

    tx.add_changed_paths(&[path("b"), path("c")]).await;
    let blocked = timeout(TEST_THROTTLE_INTERVAL / 2, throttled.recv()).await;
    assert_eq!(blocked.is_err(), true);

    let second = timeout(TEST_THROTTLE_INTERVAL * 2, throttled.recv())
        .await
        .expect("second emit timeout");
    assert_eq!(
        second,
        Some(FileWatcherEvent {
            paths: vec![path("b"), path("c")],
        })
    );
}

#[tokio::test]
async fn throttled_receiver_flushes_pending_on_shutdown() {
    let (tx, rx) = watch_channel();
    let mut throttled = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    tx.add_changed_paths(&[path("a")]).await;
    let first = timeout(Duration::from_secs(1), throttled.recv())
        .await
        .expect("first emit timeout");
    assert_eq!(
        first,
        Some(FileWatcherEvent {
            paths: vec![path("a")],
        })
    );

    tx.add_changed_paths(&[path("b")]).await;
    drop(tx);

    let second = timeout(Duration::from_secs(1), throttled.recv())
        .await
        .expect("shutdown flush timeout");
    assert_eq!(
        second,
        Some(FileWatcherEvent {
            paths: vec![path("b")],
        })
    );

    let closed = timeout(Duration::from_secs(1), throttled.recv())
        .await
        .expect("closed recv timeout");
    assert_eq!(closed, None);
}

#[tokio::test]
async fn debounced_receiver_coalesces_each_event_batch() {
    let (tx, rx) = watch_channel();
    let mut debounced = DebouncedWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    tx.add_changed_paths(&[path("a")]).await;
    let first = timeout(TEST_THROTTLE_INTERVAL * 2, debounced.recv())
        .await
        .expect("first emit timeout");
    assert_eq!(
        first,
        Some(FileWatcherEvent {
            paths: vec![path("a")],
        })
    );

    tx.add_changed_paths(&[path("c")]).await;
    let blocked = timeout(TEST_THROTTLE_INTERVAL / 2, debounced.recv()).await;
    assert_eq!(blocked.is_err(), true);

    tx.add_changed_paths(&[path("d")]).await;
    let second = timeout(TEST_THROTTLE_INTERVAL * 2, debounced.recv())
        .await
        .expect("second emit timeout");
    assert_eq!(
        second,
        Some(FileWatcherEvent {
            paths: vec![path("c"), path("d")],
        })
    );
}

#[tokio::test]
async fn debounced_receiver_flushes_pending_on_shutdown() {
    let (tx, rx) = watch_channel();
    let mut debounced = DebouncedWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    tx.add_changed_paths(&[path("a")]).await;
    drop(tx);

    let flushed = timeout(Duration::from_secs(1), debounced.recv())
        .await
        .expect("shutdown flush timeout");
    assert_eq!(
        flushed,
        Some(FileWatcherEvent {
            paths: vec![path("a")],
        })
    );

    let closed = timeout(Duration::from_secs(1), debounced.recv())
        .await
        .expect("closed recv timeout");
    assert_eq!(closed, None);
}

#[test]
fn is_mutating_event_filters_non_mutating_event_kinds() {
    assert_eq!(
        is_mutating_event(&notify_event(
            EventKind::Create(CreateKind::Any),
            vec![path("/tmp/created")]
        )),
        true
    );
    assert_eq!(
        is_mutating_event(&notify_event(
            EventKind::Modify(ModifyKind::Any),
            vec![path("/tmp/modified")]
        )),
        true
    );
    assert_eq!(
        is_mutating_event(&notify_event(
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            vec![path("/tmp/accessed")]
        )),
        false
    );
}

#[test]
fn register_dedupes_by_path_and_scope() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skills = temp_dir.path().join("skills");
    let other_skills = temp_dir.path().join("other-skills");
    std::fs::create_dir(&skills).expect("create skills dir");
    std::fs::create_dir(&other_skills).expect("create other skills dir");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, _rx) = watcher.add_subscriber();
    let _first = subscriber.register_path(skills.clone(), /*recursive*/ false);
    let _second = subscriber.register_path(skills.clone(), /*recursive*/ false);
    let _third = subscriber.register_path(skills.clone(), /*recursive*/ true);
    let _fourth = subscriber.register_path(other_skills.clone(), /*recursive*/ true);

    assert_eq!(watcher.watch_counts_for_test(&skills), Some((2, 1)));
    assert_eq!(watcher.watch_counts_for_test(&other_skills), Some((0, 1)));
}

#[test]
fn watch_registration_drop_unregisters_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skills = temp_dir.path().join("skills");
    std::fs::create_dir(&skills).expect("create skills dir");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, _rx) = watcher.add_subscriber();
    let registration = subscriber.register_path(skills.clone(), /*recursive*/ true);

    drop(registration);

    assert_eq!(watcher.watch_counts_for_test(&skills), None);
}

#[test]
fn subscriber_drop_unregisters_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skills = temp_dir.path().join("skills");
    std::fs::create_dir(&skills).expect("create skills dir");

    let watcher = Arc::new(FileWatcher::noop());
    let registration = {
        let (subscriber, _rx) = watcher.add_subscriber();
        subscriber.register_path(skills.clone(), /*recursive*/ true)
    };

    assert_eq!(watcher.watch_counts_for_test(&skills), None);
    drop(registration);
}

#[test]
fn missing_path_registers_nearest_existing_parent() {
    // Missing targets start with a bounded non-recursive parent fallback.
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let missing_file = temp_dir.path().join("FETCH_HEAD");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, _rx) = watcher.add_subscriber();
    let registration = subscriber.register_path(missing_file.clone(), /*recursive*/ false);

    assert_eq!(watcher.watch_counts_for_test(temp_dir.path()), Some((1, 0)));
    assert_eq!(watcher.watch_counts_for_test(&missing_file), None);

    drop(registration);

    assert_eq!(watcher.watch_counts_for_test(temp_dir.path()), None);
}

#[test]
fn deeply_missing_path_registers_nearest_existing_directory_ancestor() {
    // Missing nested targets skip file prefixes and keep the fallback non-recursive.
    let temp_dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(temp_dir.path().join("refs"), "not a dir").expect("write refs file");
    let missing_file = temp_dir.path().join("refs").join("heads").join("main");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, _rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(missing_file, /*recursive*/ false);

    assert_eq!(watcher.watch_counts_for_test(temp_dir.path()), Some((1, 0)));
}

#[tokio::test]
async fn receiver_closes_when_subscriber_drops() {
    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, mut rx) = watcher.add_subscriber();

    drop(subscriber);

    let closed = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("closed recv timeout");
    assert_eq!(closed, None);
}

#[test]
fn recursive_registration_downgrades_to_non_recursive_after_drop() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path().join("watched-dir");
    std::fs::create_dir(&root).expect("create root");

    let watcher = Arc::new(FileWatcher::new().expect("watcher"));
    let (subscriber, _rx) = watcher.add_subscriber();
    let non_recursive = subscriber.register_path(root.clone(), /*recursive*/ false);
    let recursive = subscriber.register_path(root.clone(), /*recursive*/ true);

    {
        let inner = watcher.inner.as_ref().expect("watcher inner");
        let inner = inner.lock().expect("inner lock");
        assert_eq!(
            inner.watched_paths.get(&root),
            Some(&RecursiveMode::Recursive)
        );
    }

    drop(recursive);

    {
        let inner = watcher.inner.as_ref().expect("watcher inner");
        let inner = inner.lock().expect("inner lock");
        assert_eq!(
            inner.watched_paths.get(&root),
            Some(&RecursiveMode::NonRecursive)
        );
    }

    drop(non_recursive);
}

#[cfg(windows)]
#[tokio::test]
async fn native_watcher_reports_changes_after_unwatch_and_rewatch() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let watcher = Arc::new(FileWatcher::new().expect("watcher"));
    let (subscriber, mut rx) = watcher.add_subscriber();

    for directory in ["first", "second"] {
        let root = temp_dir.path().join(directory);
        std::fs::create_dir(&root).expect("create watched directory");
        let registration = subscriber.register_path(root.clone(), /*recursive*/ false);
        assert!(registration.registration_succeeded());

        let changed_path = root.join("changed.txt");
        std::fs::write(&changed_path, "new content").expect("write watched file");
        let event = timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("native watch event timeout");
        assert_eq!(
            event,
            Some(FileWatcherEvent {
                paths: vec![changed_path]
            })
        );

        drop(registration);
    }
}

#[test]
fn unregister_holds_state_lock_until_unwatch_finishes() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path().join("watched-dir");
    std::fs::create_dir(&root).expect("create root");

    let watcher = Arc::new(FileWatcher::new().expect("watcher"));
    let (unregister_subscriber, _unregister_rx) = watcher.add_subscriber();
    let (register_subscriber, _register_rx) = watcher.add_subscriber();
    let registration = unregister_subscriber.register_path(root.clone(), /*recursive*/ true);

    let inner = watcher.inner.as_ref().expect("watcher inner");
    let inner_guard = inner.lock().expect("inner lock");

    let unregister_thread = std::thread::spawn(move || {
        drop(registration);
    });

    let state_lock_observed = (0..100).any(|_| {
        let locked = watcher.state.try_write().is_err();
        if !locked {
            std::thread::sleep(Duration::from_millis(10));
        }
        locked
    });
    assert_eq!(state_lock_observed, true);

    let register_root = root.clone();
    let register_thread = std::thread::spawn(move || {
        let registration =
            register_subscriber.register_path(register_root, /*recursive*/ false);
        (register_subscriber, registration)
    });

    drop(inner_guard);

    unregister_thread.join().expect("unregister join");
    let (register_subscriber, non_recursive) = register_thread.join().expect("register join");

    assert_eq!(watcher.watch_counts_for_test(&root), Some((1, 0)));

    let inner = watcher.inner.as_ref().expect("watcher inner");
    let inner = inner.lock().expect("inner lock");
    assert_eq!(
        inner.watched_paths.get(&root),
        Some(&RecursiveMode::NonRecursive)
    );
    drop(inner);

    drop(non_recursive);
    drop(register_subscriber);
}

#[tokio::test]
async fn matching_subscribers_are_notified() {
    let watcher = Arc::new(FileWatcher::noop());
    let (skills_subscriber, skills_rx) = watcher.add_subscriber();
    let (plugins_subscriber, plugins_rx) = watcher.add_subscriber();
    let _skills = skills_subscriber.register_path(path("/tmp/skills"), /*recursive*/ true);
    let _plugins = plugins_subscriber.register_path(path("/tmp/plugins"), /*recursive*/ true);
    let mut skills_rx = ThrottledWatchReceiver::new(skills_rx, TEST_THROTTLE_INTERVAL);
    let mut plugins_rx = ThrottledWatchReceiver::new(plugins_rx, TEST_THROTTLE_INTERVAL);

    watcher
        .send_paths_for_test(vec![path("/tmp/skills/rust/SKILL.md")])
        .await;

    let skills_event = timeout(Duration::from_secs(1), skills_rx.recv())
        .await
        .expect("skills change timeout")
        .expect("skills change");
    assert_eq!(
        skills_event,
        FileWatcherEvent {
            paths: vec![path("/tmp/skills/rust/SKILL.md")],
        }
    );

    let plugins_event = timeout(TEST_THROTTLE_INTERVAL, plugins_rx.recv()).await;
    assert_eq!(plugins_event.is_err(), true);
}

#[tokio::test]
async fn non_recursive_watch_ignores_grandchildren() {
    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(path("/tmp/skills"), /*recursive*/ false);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    watcher
        .send_paths_for_test(vec![path("/tmp/skills/nested/SKILL.md")])
        .await;

    let event = timeout(TEST_THROTTLE_INTERVAL, rx.recv()).await;
    assert_eq!(event.is_err(), true);
}

#[tokio::test]
async fn ancestor_events_notify_child_watches() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skills_dir = temp_dir.path().join("skills");
    let rust_dir = skills_dir.join("rust");
    let skill_file = rust_dir.join("SKILL.md");
    std::fs::create_dir(&skills_dir).expect("create skills dir");
    std::fs::create_dir(&rust_dir).expect("create rust dir");
    std::fs::write(&skill_file, "name: rust\n").expect("write skill file");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(skill_file, /*recursive*/ false);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    watcher.send_paths_for_test(vec![skills_dir.clone()]).await;

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("ancestor event timeout")
        .expect("ancestor event");
    assert_eq!(
        event,
        FileWatcherEvent {
            paths: vec![skills_dir],
        }
    );
}

#[tokio::test]
async fn missing_file_watch_reports_requested_path_when_parent_changes() {
    // Parent events for a newly-created target should report the requested file.
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let missing_file = temp_dir.path().join("FETCH_HEAD");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(missing_file.clone(), /*recursive*/ false);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    watcher
        .send_paths_for_test(vec![temp_dir.path().join("FETCH_HEAD.lock")])
        .await;
    let sibling_event = timeout(TEST_THROTTLE_INTERVAL, rx.recv()).await;
    assert_eq!(sibling_event.is_err(), true);

    std::fs::write(&missing_file, "origin/main\n").expect("write missing file");
    watcher
        .send_paths_for_test(vec![temp_dir.path().into()])
        .await;

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("missing file change timeout")
        .expect("missing file change");
    assert_eq!(
        event,
        FileWatcherEvent {
            paths: vec![missing_file],
        }
    );
}

#[tokio::test]
async fn missing_file_watch_reports_requested_path_when_parent_delete_event_arrives() {
    // Parent events should report both creation and deletion of a fallback target.
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let missing_file = temp_dir.path().join("FETCH_HEAD");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(missing_file.clone(), /*recursive*/ false);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    std::fs::write(&missing_file, "origin/main\n").expect("write missing file");
    watcher
        .send_paths_for_test(vec![temp_dir.path().into()])
        .await;
    let created = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("created event timeout")
        .expect("created event");
    assert_eq!(
        created,
        FileWatcherEvent {
            paths: vec![missing_file.clone()],
        }
    );

    std::fs::remove_file(&missing_file).expect("remove missing file");
    watcher
        .send_paths_for_test(vec![temp_dir.path().into()])
        .await;
    let deleted = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("deleted event timeout")
        .expect("deleted event");
    assert_eq!(
        deleted,
        FileWatcherEvent {
            paths: vec![missing_file],
        }
    );
}

#[tokio::test]
async fn missing_directory_watch_moves_to_created_directory_for_child_events() {
    // Missing directory watches move closer as components appear, without recursive fallback.
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let skills_dir = temp_dir.path().join("skills");
    let skill_file = skills_dir.join("SKILL.md");

    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(skills_dir.clone(), /*recursive*/ false);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);

    assert_eq!(watcher.watch_counts_for_test(temp_dir.path()), Some((1, 0)));
    assert_eq!(watcher.watch_counts_for_test(&skills_dir), None);

    std::fs::create_dir(&skills_dir).expect("create skills dir");
    watcher
        .send_paths_for_test(vec![temp_dir.path().into()])
        .await;

    let created = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("created dir event timeout")
        .expect("created dir event");
    assert_eq!(
        created,
        FileWatcherEvent {
            paths: vec![skills_dir.clone()],
        }
    );
    assert_eq!(watcher.watch_counts_for_test(temp_dir.path()), None);
    assert_eq!(watcher.watch_counts_for_test(&skills_dir), Some((1, 0)));

    std::fs::write(&skill_file, "name: rust\n").expect("write skill file");
    watcher.send_paths_for_test(vec![skill_file.clone()]).await;

    let changed_child = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("changed child event timeout")
        .expect("changed child event");
    assert_eq!(
        changed_child,
        FileWatcherEvent {
            paths: vec![skill_file],
        }
    );
}

#[tokio::test]
async fn spawn_event_loop_filters_non_mutating_events() {
    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, rx) = watcher.add_subscriber();
    let _registration = subscriber.register_path(path("/tmp/skills"), /*recursive*/ true);
    let mut rx = ThrottledWatchReceiver::new(rx, TEST_THROTTLE_INTERVAL);
    let (raw_tx, raw_rx) = mpsc::unbounded_channel();
    watcher.spawn_event_loop_for_test(raw_rx);

    raw_tx
        .send(Ok(notify_event(
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            vec![path("/tmp/skills/SKILL.md")],
        )))
        .expect("send access event");
    let blocked = timeout(TEST_THROTTLE_INTERVAL, rx.recv()).await;
    assert_eq!(blocked.is_err(), true);

    raw_tx
        .send(Ok(notify_event(
            EventKind::Create(CreateKind::File),
            vec![path("/tmp/skills/SKILL.md")],
        )))
        .expect("send create event");
    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("create event timeout")
        .expect("create event");
    assert_eq!(
        event,
        FileWatcherEvent {
            paths: vec![path("/tmp/skills/SKILL.md")],
        }
    );
}

#[tokio::test]
async fn dropping_live_watcher_releases_inner_watcher() {
    let watcher = FileWatcher::new().expect("watcher");
    let weak_inner = Arc::downgrade(watcher.inner.as_ref().expect("watcher inner"));

    drop(watcher);

    assert_eq!(weak_inner.upgrade().is_none(), true);
}

#[tokio::test]
async fn registration_reports_success_for_new_and_shared_watches()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let watcher = Arc::new(FileWatcher::new()?);
    let (subscriber, _rx) = watcher.add_subscriber();
    let (other_subscriber, _other_rx) = watcher.add_subscriber();
    // Missing directories succeed through a non-recursive watch on their existing parent.
    for path in [
        directory.path().to_path_buf(),
        directory.path().join("missing"),
    ] {
        let first = subscriber.register_path(path.clone(), /*recursive*/ true);
        let shared = other_subscriber.register_path(path.clone(), /*recursive*/ false);
        assert!(first.registration_succeeded(), "new watch for {path:?}");
        assert!(shared.registration_succeeded(), "shared watch for {path:?}");
        drop(first);
        assert_eq!(
            watcher.watch_counts_for_test(directory.path()),
            Some((1, 0))
        );
        drop(shared);
        assert_eq!(watcher.watch_counts_for_test(directory.path()), None);
    }
    Ok(())
}

#[tokio::test]
async fn registration_reports_missing_os_watches() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().canonicalize()?;
    let missing = root.join("a-missing");
    let watched = root.join("b-watched");
    std::fs::create_dir(&watched)?;
    let watcher = Arc::new(FileWatcher::new()?);
    let (subscriber, _rx) = watcher.add_subscriber();
    let paths = vec![
        WatchPath {
            path: missing.clone(),
            recursive: true,
        },
        WatchPath {
            path: watched.clone(),
            recursive: true,
        },
    ];
    // Model a path disappearing after resolution but before the OS registration.
    let registrations = paths
        .iter()
        .map(|watch| SubscriberWatchRegistration {
            key: SubscriberWatchKey {
                requested: watch.clone(),
                matched: watch.clone(),
            },
            actual: watch.clone(),
            fallback: false,
        })
        .collect::<Vec<_>>();
    assert!(!watcher.register_paths(subscriber.id, &registrations));

    std::fs::create_dir(&missing)?;
    // The directory now exists, but the unchanged mode does not retry its OS watch.
    let repeated = subscriber.register_paths(paths);
    assert!(!repeated.registration_succeeded());
    // The other path registered successfully despite the first path's failure.
    let successful = subscriber.register_path(watched.clone(), /*recursive*/ true);
    assert!(successful.registration_succeeded());
    {
        let inner = watcher
            .inner
            .as_ref()
            .ok_or("missing watcher backend")?
            .lock()
            .map_err(|_| "poisoned watcher lock")?;
        assert_eq!(
            inner.watched_paths,
            HashMap::from([(watched.clone(), RecursiveMode::Recursive)])
        );
    }
    drop(successful);
    drop(repeated);
    // Release the original registrations made directly through the subscriber ID.
    drop(subscriber);
    assert_eq!(watcher.watch_counts_for_test(&missing), None);
    assert_eq!(watcher.watch_counts_for_test(&watched), None);
    Ok(())
}

#[test]
fn noop_registration_reports_failure() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let watcher = Arc::new(FileWatcher::noop());
    let (subscriber, _rx) = watcher.add_subscriber();
    let registration =
        subscriber.register_path(directory.path().to_path_buf(), /*recursive*/ true);
    assert!(!registration.registration_succeeded());
    drop(registration);
    assert_eq!(watcher.watch_counts_for_test(directory.path()), None);
    Ok(())
}
