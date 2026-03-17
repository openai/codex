use super::*;
use notify::EventKind;
use notify::event::AccessKind;
use notify::event::AccessMode;
use notify::event::CreateKind;
use notify::event::ModifyKind;
use notify::event::RemoveKind;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

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

#[test]
fn throttles_and_coalesces_within_interval() {
    let start = Instant::now();
    let mut throttled = ThrottledPaths::new(start);

    throttled.add(vec![path("a")]);
    let first = throttled.take_ready(start).expect("first emit");
    assert_eq!(first, vec![path("a")]);

    throttled.add(vec![path("b"), path("c")]);
    assert_eq!(throttled.take_ready(start), None);

    let second = throttled
        .take_ready(start + WATCHER_THROTTLE_INTERVAL)
        .expect("coalesced emit");
    assert_eq!(second, vec![path("b"), path("c")]);
}

#[test]
fn flushes_pending_on_shutdown() {
    let start = Instant::now();
    let mut throttled = ThrottledPaths::new(start);

    throttled.add(vec![path("a")]);
    let _ = throttled.take_ready(start).expect("first emit");

    throttled.add(vec![path("b")]);
    assert_eq!(throttled.take_ready(start), None);

    let flushed = throttled
        .take_pending(start)
        .expect("shutdown flush emits pending paths");
    assert_eq!(flushed, vec![path("b")]);
}

#[test]
fn classify_event_filters_to_skills_roots() {
    let root = path("/tmp/skills");
    let state = RwLock::new(WatchState {
        skills_root_ref_counts: HashMap::from([(root.clone(), 1)]),
    });
    let event = notify_event(
        EventKind::Create(CreateKind::Any),
        vec![
            root.join("demo/SKILL.md"),
            path("/tmp/other/not-a-skill.txt"),
        ],
    );

    let classified = classify_event(&event, &state);
    assert_eq!(classified, vec![root.join("demo/SKILL.md")]);
}

#[test]
fn classify_event_supports_multiple_roots_without_prefix_false_positives() {
    let root_a = path("/tmp/skills");
    let root_b = path("/tmp/workspace/.codex/skills");
    let state = RwLock::new(WatchState {
        skills_root_ref_counts: HashMap::from([(root_a.clone(), 1), (root_b.clone(), 1)]),
    });
    let event = notify_event(
        EventKind::Modify(ModifyKind::Any),
        vec![
            root_a.join("alpha/SKILL.md"),
            path("/tmp/skills-extra/not-under-skills.txt"),
            root_b.join("beta/SKILL.md"),
        ],
    );

    let classified = classify_event(&event, &state);
    assert_eq!(
        classified,
        vec![root_a.join("alpha/SKILL.md"), root_b.join("beta/SKILL.md")]
    );
}

#[test]
fn classify_event_ignores_non_mutating_event_kinds() {
    let root = path("/tmp/skills");
    let state = RwLock::new(WatchState {
        skills_root_ref_counts: HashMap::from([(root.clone(), 1)]),
    });
    let path = root.join("demo/SKILL.md");

    let access_event = notify_event(
        EventKind::Access(AccessKind::Open(AccessMode::Any)),
        vec![path.clone()],
    );
    assert_eq!(classify_event(&access_event, &state), Vec::<PathBuf>::new());

    let any_event = notify_event(EventKind::Any, vec![path.clone()]);
    assert_eq!(classify_event(&any_event, &state), Vec::<PathBuf>::new());

    let other_event = notify_event(EventKind::Other, vec![path]);
    assert_eq!(classify_event(&other_event, &state), Vec::<PathBuf>::new());
}

#[test]
fn register_skills_root_dedupes_state_entries() {
    let watcher = FileWatcher::noop();
    let root = path("/tmp/skills");
    watcher.register_skills_root(root.clone());
    watcher.register_skills_root(root);
    watcher.register_skills_root(path("/tmp/other-skills"));

    let state = watcher.state.read().expect("state lock");
    assert_eq!(state.skills_root_ref_counts.len(), 2);
}

#[test]
fn watch_registration_drop_unregisters_roots() {
    let watcher = Arc::new(FileWatcher::noop());
    let root = path("/tmp/skills");
    watcher.register_skills_root(root.clone());
    let registration = WatchRegistration {
        file_watcher: Arc::downgrade(&watcher),
        roots: vec![root],
    };

    drop(registration);

    let state = watcher.state.read().expect("state lock");
    assert_eq!(state.skills_root_ref_counts.len(), 0);
}

#[test]
fn unregister_holds_state_lock_until_unwatch_finishes() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let root = temp_dir.path().join("skills");
    std::fs::create_dir(&root).expect("create root");

    let watcher = Arc::new(FileWatcher::new(temp_dir.path().to_path_buf()).expect("watcher"));
    watcher.register_skills_root(root.clone());

    let inner = watcher.inner.as_ref().expect("watcher inner");
    let inner_guard = inner.lock().expect("inner lock");

    let unregister_watcher = Arc::clone(&watcher);
    let unregister_root = root.clone();
    let unregister_thread = std::thread::spawn(move || {
        unregister_watcher.unregister_roots(&[unregister_root]);
    });

    let state_lock_observed = (0..100).any(|_| {
        let locked = watcher.state.try_write().is_err();
        if !locked {
            std::thread::sleep(Duration::from_millis(10));
        }
        locked
    });
    assert_eq!(state_lock_observed, true);

    let register_watcher = Arc::clone(&watcher);
    let register_root = root.clone();
    let register_thread = std::thread::spawn(move || {
        register_watcher.register_skills_root(register_root);
    });

    drop(inner_guard);

    unregister_thread.join().expect("unregister join");
    register_thread.join().expect("register join");

    let state = watcher.state.read().expect("state lock");
    assert_eq!(state.skills_root_ref_counts.get(&root), Some(&1));
    drop(state);

    let inner = watcher.inner.as_ref().expect("watcher inner");
    let inner = inner.lock().expect("inner lock");
    assert_eq!(
        inner.watched_paths.get(&root),
        Some(&RecursiveMode::Recursive)
    );
}

#[tokio::test]
async fn spawn_event_loop_flushes_pending_changes_on_shutdown() {
    let watcher = FileWatcher::noop();
    let root = path("/tmp/skills");
    {
        let mut state = watcher.state.write().expect("state lock");
        state.skills_root_ref_counts.insert(root.clone(), 1);
    }

    let (raw_tx, raw_rx) = mpsc::unbounded_channel();
    let (tx, mut rx) = broadcast::channel(8);
    watcher.spawn_event_loop(raw_rx, Arc::clone(&watcher.state), tx);

    raw_tx
        .send(Ok(notify_event(
            EventKind::Create(CreateKind::File),
            vec![root.join("a/SKILL.md")],
        )))
        .expect("send first event");
    let first = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("first watcher event")
        .expect("broadcast recv first");
    assert_eq!(
        first,
        FileWatcherEvent::SkillsChanged {
            paths: vec![root.join("a/SKILL.md")]
        }
    );

    raw_tx
        .send(Ok(notify_event(
            EventKind::Remove(RemoveKind::File),
            vec![root.join("b/SKILL.md")],
        )))
        .expect("send second event");
    drop(raw_tx);

    let second = timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("second watcher event")
        .expect("broadcast recv second");
    assert_eq!(
        second,
        FileWatcherEvent::SkillsChanged {
            paths: vec![root.join("b/SKILL.md")]
        }
    );
}
