use pretty_assertions::assert_eq;

use crate::BrowserColor;
use crate::BrowserLaunchContext;
use crate::BrowserNetworkPolicy;
use crate::HumanNavigationAction;
use crate::TerminalBrowser;
use crate::TerminalSize;
use crate::actions::bounded_snapshot_json;
use crate::handles::BrowserHandles;
use crate::human_control::HumanControlStateTransition;
use crate::process::carbonyl_args;
use crate::screen::TerminalQueryResponder;
use crate::screen::TerminalScreen;
use crate::session::RenderMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn open_requires_network_enabled_by_the_active_permission_profile() {
    let browser = TerminalBrowser::discover();
    let error = browser
        .execute(
            "test-session",
            "open",
            serde_json::json!({ "url": "https://example.com" }),
        )
        .await
        .expect_err("disabled network policy should reject browser navigation");

    assert_eq!(
        error.to_string(),
        "terminal browser network access is disabled by the active permission profile"
    );
}

#[tokio::test]
async fn emergency_termination_prevents_future_browser_startup() {
    let browser = TerminalBrowser::discover();
    browser
        .set_network_policy(BrowserNetworkPolicy::Direct)
        .await;

    browser.terminate();

    let error = browser
        .execute(
            "test-session",
            "open",
            serde_json::json!({ "url": "https://example.com" }),
        )
        .await
        .expect_err("terminated browser should reject startup");
    assert_eq!(error.to_string(), "terminal browser has been terminated");
}

#[tokio::test]
async fn model_actions_are_rejected_during_human_control() {
    let browser = TerminalBrowser::discover();
    browser
        .inner
        .human_control
        .store(/*val*/ true, Ordering::SeqCst);

    let error = browser
        .execute("test-session", "snapshot", serde_json::json!({}))
        .await
        .expect_err("model action should be rejected");

    assert!(error.to_string().contains("human_control_active"));
}

#[tokio::test]
async fn crash_exit_from_human_control_flushes_pending_handle_invalidation() {
    let browser = TerminalBrowser::discover();
    browser
        .inner
        .human_control
        .store(/*val*/ true, Ordering::SeqCst);

    browser.inner.set_crashed("input disconnected".to_string());

    assert!(!browser.is_human_control_active());
    assert!(
        browser
            .inner
            .human_handle_invalidation_pending
            .load(Ordering::SeqCst)
    );
    let _ = browser
        .execute("test-session", "snapshot", serde_json::json!({}))
        .await
        .expect_err("browser session is not open");
    assert!(
        !browser
            .inner
            .human_handle_invalidation_pending
            .load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn hiding_the_panel_ends_human_control_without_relocking_tool_dispatch() {
    let browser = TerminalBrowser::discover();
    *browser
        .inner
        .session_key
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some("test-session".to_string());
    browser
        .inner
        .human_control
        .store(/*val*/ true, Ordering::SeqCst);
    browser.inner.update_view(|view| {
        view.visible = true;
        view.human_control = true;
    });

    tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 1),
        browser.execute(
            "test-session",
            "set_visibility",
            serde_json::json!({ "visible": false }),
        ),
    )
    .await
    .expect("visibility update must not deadlock")
    .expect("visibility update should succeed");

    assert!(!browser.is_human_control_active());
    let view = browser.view();
    assert!(!view.visible);
    assert!(!view.human_control);
}

#[tokio::test]
async fn hiding_the_panel_invalidates_a_queued_human_control_request() {
    let browser = TerminalBrowser::discover();
    let token = browser.human_control_token();
    browser.set_visibility(/*visible*/ false);

    let error = browser
        .toggle_human_control(token)
        .await
        .expect_err("hidden panel should cancel the queued request");

    assert_eq!(error.to_string(), "browser control transition was canceled");
    assert!(!browser.is_human_control_active());
    let view = browser.view();
    assert!(!view.visible);
    assert!(!view.human_control);
}

#[test]
fn hiding_the_panel_wins_a_race_with_a_claimed_human_control_generation() {
    for _ in 0..32 {
        let browser = TerminalBrowser::discover();
        browser.set_visibility(/*visible*/ true);
        let control_generation = browser
            .inner
            .human_control_generation
            .load(Ordering::SeqCst);
        let active_generation = control_generation.wrapping_add(/*rhs*/ 1);
        browser
            .inner
            .human_control_generation
            .compare_exchange(
                /*current*/ control_generation,
                /*new*/ active_generation,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .expect("claim control generation");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(/*n*/ 2));
        let activation_browser = browser.clone();
        let activation_barrier = std::sync::Arc::clone(&barrier);
        let activation = std::thread::spawn(move || {
            activation_barrier.wait();
            activation_browser.inner.transition_human_control(
                HumanControlStateTransition::Activate {
                    generation: active_generation,
                },
            )
        });

        barrier.wait();
        browser.set_visibility(/*visible*/ false);
        let _ = activation.join().expect("activation thread");

        assert!(!browser.is_human_control_active());
        let view = browser.view();
        assert!(!view.visible);
        assert!(!view.human_control);
    }
}

#[test]
fn terminal_browser_ignores_duplicate_resize_events() {
    let browser = TerminalBrowser::discover();
    let mut resize_rx = browser.inner.resize_tx.subscribe();
    let size = TerminalSize {
        rows: 40,
        cols: 120,
    };

    browser.resize(size).expect("first resize should succeed");
    assert!(resize_rx.has_changed().expect("resize sender is open"));
    assert_eq!(*resize_rx.borrow_and_update(), size);

    browser
        .resize(size)
        .expect("duplicate resize should succeed");
    assert!(!resize_rx.has_changed().expect("resize sender is open"));
}

#[tokio::test]
async fn model_profile_requests_do_not_mutate_without_user_approval() {
    let root = tempfile::tempdir().expect("test root");
    let root = AbsolutePathBuf::from_absolute_path(root.path()).expect("absolute test root");
    let browser = TerminalBrowser::discover_with_launch_context(BrowserLaunchContext {
        codex_home: Some(root.join("codex-home")),
        workspace_root: Some(root.join("workspace")),
        ..Default::default()
    });

    let output = browser
        .execute(
            "test-session",
            "profile",
            serde_json::json!({ "action": "requestCreate", "name": "work" }),
        )
        .await
        .expect("profile request");
    let crate::BrowserToolOutput::Text(output) = output else {
        panic!("expected text output");
    };
    let output: serde_json::Value = serde_json::from_str(&output).expect("approval JSON");

    assert_eq!(output["status"], "approvalRequired");
    assert_eq!(output["command"], "/browser profile create work");
    assert!(browser.profiles().expect("profile listing").is_empty());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test]
#[ignore = "opt-in smoke test requiring CODEX_CARBONYL_BINARY and a host sandbox"]
async fn configured_real_carbonyl_opens_local_page_and_snapshots() {
    use std::io::Read;
    use std::io::Write;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind smoke server");
    listener
        .set_nonblocking(/*nonblocking*/ true)
        .expect("configure smoke server");
    let address = listener.local_addr().expect("smoke server address");
    let server = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 15);
        let mut remaining_requests = 2;
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(/*secs*/ 2)))
                        .expect("set smoke read timeout");
                    let mut request = [0_u8; 2_048];
                    let _ = stream.read(&mut request);
                    let body =
                        "<!doctype html><title>Carbonyl smoke</title><button>Smoke button</button>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write smoke response");
                    remaining_requests -= 1;
                    if remaining_requests == 0 {
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "Carbonyl did not connect to the smoke server"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(/*millis*/ 10));
                }
                Err(error) => panic!("accept smoke request: {error}"),
            }
        }
    });
    let browser = TerminalBrowser::discover();
    browser
        .resize(TerminalSize {
            rows: 40,
            cols: 120,
        })
        .expect("enlarge startup diagnostics");
    browser
        .set_network_policy(BrowserNetworkPolicy::Direct)
        .await;

    let open_result = browser
        .execute(
            "real-smoke",
            "open",
            serde_json::json!({
                "url": format!("http://{address}"),
                "visible": false,
            }),
        )
        .await;
    if let Err(error) = open_result {
        let view = browser.view();
        let screen = (0..view.screen.rows)
            .map(|row| {
                (0..view.screen.cols)
                    .filter_map(|col| view.screen.cell(row, col))
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        panic!("open local smoke page: {error}\nCarbonyl output:\n{screen}");
    }
    let snapshot = browser
        .execute("real-smoke", "snapshot", serde_json::json!({}))
        .await
        .expect("snapshot local smoke page");
    let crate::BrowserToolOutput::Text(snapshot) = snapshot else {
        panic!("expected text snapshot");
    };

    assert!(snapshot.contains("Smoke button"));
    let render_deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 5);
    loop {
        let view = browser.view();
        let rendered = (0..view.screen.rows)
            .map(|row| {
                (0..view.screen.cols)
                    .filter_map(|col| view.screen.cell(row, col))
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        if rendered.contains("Smoke button") {
            break;
        }
        assert!(
            std::time::Instant::now() < render_deadline,
            "Carbonyl did not render the smoke page in its PTY:\n{rendered}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 20)).await;
    }
    let token = browser
        .toggle_human_control(browser.human_control_token())
        .await
        .expect("begin smoke-test human control");
    browser
        .navigate_for_human(HumanNavigationAction::Reload)
        .await
        .expect("reload from the Codex-owned browser chrome");
    browser
        .end_human_control(token)
        .await
        .expect("end smoke-test human control");
    browser.close().await;
    server.join().expect("smoke server thread");
}

#[tokio::test]
async fn terminal_browser_resize_updates_coalesce_to_the_latest_size() {
    let browser = TerminalBrowser::discover();
    let mut resize_rx = browser.inner.resize_tx.subscribe();
    let sizes = [
        TerminalSize {
            rows: 31,
            cols: 101,
        },
        TerminalSize {
            rows: 32,
            cols: 102,
        },
        TerminalSize {
            rows: 33,
            cols: 103,
        },
    ];

    for size in sizes {
        browser.resize(size).expect("resize should succeed");
    }

    resize_rx.changed().await.expect("resize sender is open");
    assert_eq!(*resize_rx.borrow_and_update(), sizes[2]);
    assert!(!resize_rx.has_changed().expect("resize sender is open"));
}

#[test]
fn terminal_screen_preserves_styles_wide_cells_cursor_and_title() {
    let mut terminal = TerminalScreen::new(TerminalSize { rows: 2, cols: 8 });
    terminal.process(b"\x1b]2;Example");
    terminal.process(b"\x07\x1b[31;44;1;3;4m");
    terminal.process("\u{754c}".as_bytes());

    let screen = terminal.snapshot();
    let cell = screen.cell(/*row*/ 0, /*col*/ 0).expect("first cell");
    assert_eq!(cell.text, "\u{754c}");
    assert_eq!(cell.foreground, BrowserColor::Indexed(1));
    assert_eq!(cell.background, BrowserColor::Indexed(4));
    assert!(cell.bold);
    assert!(cell.italic);
    assert!(cell.underlined);
    assert!(
        screen
            .cell(/*row*/ 0, /*col*/ 1)
            .expect("wide continuation")
            .wide_continuation
    );
    assert_eq!(screen.cursor, Some((0, 2)));
    assert_eq!(terminal.title().as_deref(), Some("Example"));
}

#[test]
fn terminal_query_responder_handles_queries_split_across_chunks() {
    let mut responder = TerminalQueryResponder::default();
    assert_eq!(responder.process(b"prefix\x1bP$q"), Vec::<Vec<u8>>::new());
    assert_eq!(
        responder.process(b"m\x1b\\suffix"),
        vec![b"\x1bP1$r48:2:0:0:0m\x1b\\".to_vec()]
    );
    assert_eq!(responder.process(b"\x1bP+q54"), Vec::<Vec<u8>>::new());
    assert_eq!(
        responder.process(b"4e\x1b\\"),
        vec![b"\x1bP1+r544e=787465726d2d323536636f6c6f72\x1b\\".to_vec()]
    );
}

#[test]
fn carbonyl_uses_the_managed_proxy_only_when_policy_provides_one() {
    let direct = carbonyl_args(
        "/tmp/profile",
        &BrowserNetworkPolicy::Direct,
        RenderMode::NativeText,
    );
    assert!(!direct.iter().any(|arg| arg.starts_with("--proxy-server=")));

    let http_addr = "127.0.0.1:43128".parse().expect("valid proxy address");
    let proxied = carbonyl_args(
        "/tmp/profile",
        &BrowserNetworkPolicy::ManagedProxy { http_addr },
        RenderMode::NativeText,
    );
    assert!(proxied.contains(&"--proxy-server=http://127.0.0.1:43128".to_string()));
    assert!(proxied.contains(&"--proxy-bypass-list=<-loopback>".to_string()));
}

#[test]
fn browser_node_handles_reject_unknown_and_stale_ids() {
    let mut handles = BrowserHandles::default();
    let node_id = handles.insert(/*backend_node_id*/ 42);
    assert_eq!(handles.resolve(&node_id).expect("current handle"), 42);
    assert!(handles.resolve("body > button").is_err());
    handles.clear();
    assert!(handles.resolve(&node_id).is_err());
}

#[test]
fn serialized_snapshot_has_a_hard_output_cap_and_remains_valid_json() {
    let nodes = (0..100)
        .map(|index| {
            serde_json::json!({
                "nodeId": format!("d0123456789abcdefn{index}"),
                "text": "x".repeat(/*n*/ 1_000),
                "value": "y".repeat(/*n*/ 1_000),
            })
        })
        .collect::<Vec<_>>();
    let output = bounded_snapshot_json(serde_json::json!({
        "url": format!("https://example.com/{}", "u".repeat(/*n*/ 100_000)),
        "title": "t".repeat(/*n*/ 100_000),
        "nodes": nodes,
        "text": "body ".repeat(/*n*/ 10_000),
    }))
    .unwrap();

    assert!(output.len() <= 8 * 1024);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.get("truncated"),
        Some(&serde_json::Value::Bool(true))
    );
}
