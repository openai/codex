use super::*;
use pretty_assertions::assert_eq;

#[test]
fn policy_preserves_explicit_modes_and_terminal_ownership() {
    use VscodeDetection::Other;
    use VscodeDetection::Unknown;
    use VscodeDetection::VsCode;
    for (platform_default, ssh, wsl, vscode, expected) in [
        (true, false, false, Other, [false, true, true]),
        (false, false, false, Other, [false, true, false]),
        (true, true, false, Other, [false, false, false]),
        (true, false, false, VsCode, [false, false, false]),
        (true, false, true, Unknown, [false, true, false]),
        (true, false, true, Other, [false, true, true]),
    ] {
        let env = PasteEnvironment {
            platform_default,
            ssh,
            wsl,
            vscode,
        };
        assert_eq!(
            [
                RightClickPaste::Off,
                RightClickPaste::On,
                RightClickPaste::Auto
            ]
            .map(|mode| env.allows(mode)),
            expected
        );
    }
}
