//! The configuration boundary shares URI resolution and rejects lossy denials.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn configuration_paths_resolve_with_the_supplied_platform_and_home() {
    for (convention, base, home, input, expected) in [
        (
            PathConvention::Posix,
            "file:///work/project",
            "file:///home/user",
            "~/a b/雪%/",
            "/home/user/a b/雪%",
        ),
        (
            PathConvention::Windows,
            "file:///C:/work/project",
            "file:///C:/Users/user",
            r"..\private\",
            r"C:\work\private",
        ),
        (
            PathConvention::Windows,
            "file:///C:/work/project",
            "file:///C:/Users/user",
            r"~\a b\雪%\",
            r"C:\Users\user\a b\雪%",
        ),
        (
            PathConvention::Windows,
            "file:///C:/work",
            "file:///C:/Users/user",
            r"\\server\share",
            r"\\server\share\",
        ),
    ] {
        let base = PathUri::parse(base).unwrap();
        let home = PathUri::parse(home).unwrap();
        assert_eq!(
            PathUri::resolve_config_path(input, convention, Some(&base), Some(&home)),
            Ok(expected.to_string()),
            "{input:?}",
        );
        if !input.starts_with('.') && !input.starts_with('\\') || input.starts_with(r"\\") {
            assert_eq!(
                PathUri::resolve_config_path(input, convention, /*base*/ None, Some(&home)),
                Ok(expected.to_string()),
                "without base: {input:?}",
            );
        }
    }
}

#[test]
fn configuration_paths_reject_ambiguous_or_lossy_denials() {
    for (convention, base) in [
        (PathConvention::Posix, "file:///base/%FF"),
        (PathConvention::Posix, "file:///base/encoded%2Fname"),
        (PathConvention::Posix, "file:///C:/base"),
        (PathConvention::Windows, "file:///base"),
        (PathConvention::Windows, "file:///C:/encoded%5Cname"),
        (PathConvention::Windows, "file:///C:/%FF"),
        (PathConvention::Windows, "file:///C:/base/a:b"),
    ] {
        let base = PathUri::parse(base).unwrap();
        assert!(base.validate_config_path(convention).is_err());
    }
    let base = PathUri::parse("file:///base").unwrap();
    let opaque = PathUri::from_opaque_path_bytes(b"/base");
    for input in ["nul\0", "nul\0/../private", "/nul\0", "~/private"] {
        assert!(
            PathUri::resolve_config_path(
                input,
                PathConvention::Posix,
                Some(&base),
                /*user_home_dir*/ None
            )
            .is_err()
        );
    }
    assert!(
        PathUri::resolve_config_path(
            "child",
            PathConvention::Posix,
            Some(&opaque),
            /*user_home_dir*/ None
        )
        .is_err()
    );
    for input in [
        r"\\server\.\private",
        r"\\server\..\private",
        r"C:\base\a:b\private",
        r"C:\base\a:b\..\private",
    ] {
        assert!(
            PathUri::resolve_config_path(
                input,
                PathConvention::Windows,
                /*base*/ None,
                /*user_home_dir*/ None
            )
            .is_err()
        );
    }
    let home = PathUri::parse("file:///C:/Users/user").unwrap();
    for input in [
        "C:",
        r"~/\private",
        r"~\/private",
        r"\\0x7f000001\share\private",
        r"\\bücher\share\private",
    ] {
        assert!(
            PathUri::resolve_config_path(input, PathConvention::Windows, Some(&home), Some(&home))
                .is_err()
        );
    }
    // Unused facts cannot change an already absolute path.
    assert_eq!(
        PathUri::resolve_config_path(
            "/private",
            PathConvention::Posix,
            Some(&opaque),
            Some(&opaque)
        ),
        Ok("/private".to_string()),
    );
}
