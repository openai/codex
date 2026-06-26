use pretty_assertions::assert_eq;

use crate::BrowserColor;
use crate::BrowserNetworkPolicy;
use crate::TerminalBrowser;
use crate::TerminalSize;
use crate::actions::bounded_snapshot_json;
use crate::process::carbonyl_args;
use crate::process::validated_websocket_url;
use crate::screen::TerminalQueryResponder;
use crate::screen::TerminalScreen;
use crate::scripts;
use crate::session::RenderMode;

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
        /*debugging_port*/ 9_222,
        "/tmp/profile",
        &BrowserNetworkPolicy::Direct,
        RenderMode::NativeText,
    );
    assert!(!direct.iter().any(|arg| arg.starts_with("--proxy-server=")));

    let http_addr = "127.0.0.1:43128".parse().expect("valid proxy address");
    let proxied = carbonyl_args(
        /*debugging_port*/ 9_222,
        "/tmp/profile",
        &BrowserNetworkPolicy::ManagedProxy { http_addr },
        RenderMode::NativeText,
    );
    assert!(proxied.contains(&"--proxy-server=http://127.0.0.1:43128".to_string()));
    assert!(proxied.contains(&"--proxy-bypass-list=<-loopback>".to_string()));
}

#[test]
fn carbonyl_devtools_websocket_must_match_the_private_loopback_listener() {
    assert_eq!(
        validated_websocket_url(
            "ws://127.0.0.1:9222/devtools/page/1",
            /*expected_port*/ 9_222,
        )
        .unwrap(),
        "ws://127.0.0.1:9222/devtools/page/1"
    );
    assert!(
        validated_websocket_url(
            "ws://192.0.2.10:9222/devtools/page/1",
            /*expected_port*/ 9_222,
        )
        .is_err()
    );
    assert!(
        validated_websocket_url(
            "ws://127.0.0.1:9223/devtools/page/1",
            /*expected_port*/ 9_222,
        )
        .is_err()
    );
    assert!(
        validated_websocket_url(
            "wss://127.0.0.1:9222/devtools/page/1",
            /*expected_port*/ 9_222,
        )
        .is_err()
    );
}

#[test]
fn browser_script_node_ids_are_validated() {
    assert!(scripts::click_expression("d0123456789abcdefn42").is_ok());
    assert!(scripts::click_expression("n42").is_err());
    assert!(scripts::click_expression("body > button").is_err());
    assert!(
        scripts::click_expression(&format!("d0123456789abcdefn{}", "1".repeat(/*n*/ 21))).is_err()
    );
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

    assert!(output.len() <= 32 * 1024);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        parsed.get("truncated"),
        Some(&serde_json::Value::Bool(true))
    );
}
