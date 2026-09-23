//! Cover shared probing, session targeting, and conservative behavior for unknown mouse settings.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn input_probe_targets_the_containing_pane_and_only_disables_confirmed_mouse_off() {
    for (mouse, mouse_capture) in [
        ("0", MouseCapture::DisabledByTmux),
        ("off", MouseCapture::DisabledByTmux),
        ("1", MouseCapture::Enabled),
        ("on", MouseCapture::Enabled),
        ("", MouseCapture::Enabled),
        ("unknown", MouseCapture::Enabled),
    ] {
        let mut calls = Vec::new();
        let options = read_options(Some("%42"), |args| {
            calls.push(args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
            Some(format!("csi-u\t{mouse}\n").into_bytes())
        });
        assert_eq!(
            options,
            Options {
                extended_keys_format: Some("csi-u".to_owned()),
                mouse_capture,
            }
        );
        assert_eq!(
            calls,
            vec![vec![
                "display-message",
                "-p",
                "-t",
                "%42",
                "#{extended-keys-format}\t#{mouse}",
            ]]
        );
    }
}

#[test]
fn keyboard_fallback_preserves_confirmed_mouse_policy_without_guessing_on_failure() {
    for (output, mouse_capture) in [
        (Some(b"\t0\n".to_vec()), MouseCapture::DisabledByTmux),
        (Some(b"\t\n".to_vec()), MouseCapture::Enabled),
        (Some(vec![0xff]), MouseCapture::Enabled),
        (None, MouseCapture::Enabled),
    ] {
        let mut calls = Vec::new();
        let options = read_options(/*pane*/ None, |args| {
            calls.push(args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
            if args[0] == "display-message" {
                output.clone()
            } else {
                Some(b"xterm\n".to_vec())
            }
        });
        assert_eq!(
            options,
            Options {
                extended_keys_format: Some("xterm".to_owned()),
                mouse_capture,
            }
        );
        assert_eq!(
            calls,
            vec![
                vec!["display-message", "-p", "#{extended-keys-format}\t#{mouse}"],
                vec!["show-options", "-gqv", "extended-keys-format"],
            ]
        );
    }
    assert_eq!(read_options(Some("%42"), |_| None), Options::default());
}
