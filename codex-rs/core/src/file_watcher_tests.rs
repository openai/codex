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
    let watcher = Arc::new(FileWatcher::noop());
    let subscriber = watcher.add_subscriber();
    let _first = subscriber.register_path(path("/tmp/skills"), false);
    let _second = subscriber.register_path(path("/tmp/skills"), false);
    let _third = subscriber.register_path(path("/tmp/skills"), true);
    let _fourth = subscriber.register_path(path("/tmp/other-skills"), true);

    assert_eq!(
        watcher.watch_counts_for_test(&path("/tmp/skills")),
        Some((2, 1))
    );
    assert_eq!(
        watcher.watch_counts_for_test(&path("/tmp/other-skills")),
        Some((0, 1))
    );
}

#[test]
fn watch_registration_drop_unregisters_paths() {
    let watcher = Arc::new(FileWatcher::noop());
    let subscriber = watcher.add_subscriber();
    let registration = subscriber.register_path(path("/tmp/skills"), true);

    drop(registration);

    assert_eq!(watcher.watch_counts_for_test(&path("/tmp/skills")), None);
}

#[test]
fn subscriber_drop_unregisters_paths() {
    let watcher = Arc::new(FileWatcher::noop());
    let registration = {
        let subscriber = watcher.add_subscriber();
        subscriber.register_path(path("/tmp/skills"), true)
    };

    assert_eq!(watcher.watch_counts_for_test(&path("/tmp/skills")), None);
    drop(registration);
}

#[test]
fn receiver_can_only_be_taken_once() {
    let watcher = Arc::new(FileWatcher::noop());
    let mut subscriber = watcher.add_subscriber();

    assert_eq!(subscriber.take_receiver().is_some(), true);
    assert_eq!(subscriber.take_receiver().is_none(), true);
}

#[test]
fn recursive_registration_downgrades_to_non_recursive_after_drop() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path().join("watched-dir");
    std::fs::create_dir(&root).expect("create root");

    let watcher = Arc::new(FileWatcher::new().expect("watcher"));
    let subscriber = watcher.add_subscriber();
    let non_recursive = subscriber.register_path(root.clone(), false);
    let recursive = subscriber.register_path(root.clone(), true);

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

#[test]
fn unregister_holds_state_lock_until_unwatch_finishes() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path().join("watched-dir");
    std::fs::create_dir(&root).expect("create root");

    let watcher = Arc::new(FileWatcher::new().expect("watcher"));
    let unregister_subscriber = watcher.add_subscriber();
    let register_subscriber = watcher.add_subscriber();
    let registration = unregister_subscriber.register_path(root.clone(), true);

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
        let registration = register_subscriber.register_path(register_root, false);
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
    let mut skills_subscriber = watcher.add_subscriber();
    let mut plugins_subscriber = watcher.add_subscriber();
    let _skills = skills_subscriber.register_path(path("/tmp/skills"), true);
    let _plugins = plugins_subscriber.register_path(path("/tmp/plugins"), true);
    let mut skills_rx = ThrottledWatchReceiver::new(
        skills_subscriber.take_receiver().expect("skills receiver"),
        TEST_THROTTLE_INTERVAL,
    );
    let mut plugins_rx = ThrottledWatchReceiver::new(
        plugins_subscriber
            .take_receiver()
            .expect("plugins receiver"),
        TEST_THROTTLE_INTERVAL,
    );

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
    let mut subscriber = watcher.add_subscriber();
    let _registration = subscriber.register_path(path("/tmp/skills"), false);
    let mut rx = ThrottledWatchReceiver::new(
        subscriber.take_receiver().expect("subscriber receiver"),
        TEST_THROTTLE_INTERVAL,
    );

    watcher
        .send_paths_for_test(vec![path("/tmp/skills/nested/SKILL.md")])
        .await;

    let event = timeout(TEST_THROTTLE_INTERVAL, rx.recv()).await;
    assert_eq!(event.is_err(), true);
}

#[tokio::test]
async fn ancestor_events_notify_child_watches() {
    let watcher = Arc::new(FileWatcher::noop());
    let mut subscriber = watcher.add_subscriber();
    let _registration = subscriber.register_path(path("/tmp/skills/rust/SKILL.md"), false);
    let mut rx = ThrottledWatchReceiver::new(
        subscriber.take_receiver().expect("subscriber receiver"),
        TEST_THROTTLE_INTERVAL,
    );

    watcher.send_paths_for_test(vec![path("/tmp/skills")]).await;

    let event = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("ancestor event timeout")
        .expect("ancestor event");
    assert_eq!(
        event,
        FileWatcherEvent {
            paths: vec![path("/tmp/skills")],
        }
    );
}

#[tokio::test]
async fn spawn_event_loop_filters_non_mutating_events() {
    let watcher = Arc::new(FileWatcher::noop());
    let mut subscriber = watcher.add_subscriber();
    let _registration = subscriber.register_path(path("/tmp/skills"), true);
    let mut rx = ThrottledWatchReceiver::new(
        subscriber.take_receiver().expect("subscriber receiver"),
        TEST_THROTTLE_INTERVAL,
    );
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
