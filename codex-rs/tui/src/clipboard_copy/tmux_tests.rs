//! Verifies tmux client selection and clipboard capability checks.

use super::clipboard_target;
use pretty_assertions::assert_eq;

#[test]
fn tmux_clipboard_copy_ready_accepts_forwarding_configuration() {
    let result = clipboard_target(
        || Ok("external\n".to_string()),
        || Ok("42 /dev/pts/7\n1 /dev/pts/8\n".to_string()),
        |client| {
            assert_eq!(client, "/dev/pts/7");
            Ok("193: Ms: (string) \\033]52;%p1%s;%p2%s\\a\n".to_string())
        },
    );

    assert_eq!(result, Ok("/dev/pts/7".to_string()));
}

#[test]
fn tmux_clipboard_copy_ready_rejects_disabled_forwarding() {
    let result = clipboard_target(
        || Ok("off\n".to_string()),
        || panic!("client should not be queried when forwarding is disabled"),
        |_| panic!("tmux info should not be queried when forwarding is disabled"),
    );

    assert_eq!(
        result,
        Err("tmux clipboard forwarding is disabled".to_string())
    );
}

#[test]
fn tmux_clipboard_target_requires_nonempty_ms_capability() {
    for info in ["", "193: Ms: [missing]\n", "193: Ms: (string) \n"] {
        let result = clipboard_target(
            || Ok("external\n".to_string()),
            || Ok("42 /dev/pts/7\n".to_string()),
            |client| {
                assert_eq!(client, "/dev/pts/7");
                Ok(info.to_string())
            },
        );

        assert_eq!(
            result,
            Err("tmux clipboard forwarding is unavailable: missing Ms capability".to_string())
        );
    }
}

#[test]
fn tmux_clipboard_target_rejects_no_attached_client() {
    let result = clipboard_target(
        || Ok("external\n".to_string()),
        || Ok("\n".to_string()),
        |_| panic!("capabilities require an attached client"),
    );
    insta::assert_snapshot!(result.expect_err("no receiving client"));
}
