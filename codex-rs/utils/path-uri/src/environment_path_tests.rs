use super::*;
use pretty_assertions::assert_eq;

fn canonical(path: &str) -> EnvironmentPath {
    EnvironmentPath(path.to_string())
}

#[test]
fn normalized_paths_collapse_separators_and_dot_segments() {
    for (input, expected) in [
        ("/", "/"),
        ("///", "/"),
        ("/workspace//src///lib.rs", "/workspace/src/lib.rs"),
        ("/workspace/./src/../lib.rs", "/workspace/lib.rs"),
        ("/../../workspace", "/workspace"),
        ("/workspace/../../../src", "/src"),
        ("/C:/Users/../Alice", "/c:/Alice"),
        ("/c:/../../Windows", "/c:/Windows"),
        ("//server/share/src/..", "//server/share"),
        (
            "//server/share/../../outside/file.rs",
            "//server/share/outside/file.rs",
        ),
    ] {
        assert_eq!(
            EnvironmentPath::new(input),
            Ok(canonical(expected)),
            "normalizing {input}"
        );
    }
}

#[test]
fn normalization_is_idempotent() {
    for input in [
        "/",
        "/workspace/src",
        "/C:",
        "/c:/Users/Alice",
        "//server/share/src",
    ] {
        let path = EnvironmentPath::new(input).expect("valid path");
        assert_eq!(EnvironmentPath::new(path.as_str()), Ok(path));
    }
}

#[test]
fn posix_paths_preserve_windows_looking_names() {
    for (input, expected) in [
        ("/C:", "/C:"),
        ("/C:/Project", "/C:/Project"),
        ("/C:/../Project", "/Project"),
        ("//server", "//server"),
    ] {
        assert_eq!(
            EnvironmentPath::posix(input),
            Ok(canonical(expected)),
            "normalizing {input}"
        );
    }
}

#[test]
fn canonical_paths_reject_unc_roots_with_dot_segments() {
    for input in ["//./share", "//../share", "//server/../file"] {
        assert!(EnvironmentPath::new(input).is_err(), "accepting {input}");
    }
}

#[test]
fn posix_paths_preserve_backslashes_and_unicode_as_filename_characters() {
    assert_eq!(
        EnvironmentPath::posix("/workspace/a\\b/日本語.rs"),
        Ok(canonical("/workspace/a\\b/日本語.rs"))
    );
}

#[test]
fn native_path_constructor_uses_the_current_host_flavor() {
    let (input, expected) = if cfg!(windows) {
        (r"C:\workspace\src", "/c:/workspace/src")
    } else {
        ("/workspace/src", "/workspace/src")
    };

    assert_eq!(
        EnvironmentPath::from_native_path(input),
        Ok(canonical(expected))
    );
}

#[test]
fn windows_drive_paths_normalize_on_every_host() {
    for (input, expected) in [
        (r"C:\Users\Alice\src", "/c:/Users/Alice/src"),
        ("D:/work/src", "/d:/work/src"),
        (r"E:\work\\src\.\lib\..\main.rs", "/e:/work/src/main.rs"),
        (r"z:\..\..\Windows", "/z:/Windows"),
        (r"C:\", "/c:/"),
    ] {
        assert_eq!(
            EnvironmentPath::windows(input),
            Ok(canonical(expected)),
            "normalizing {input}"
        );
    }
}

#[test]
fn windows_unc_paths_normalize_on_every_host() {
    for (input, expected) in [
        (r"\\server\share", "//server/share"),
        (r"\\server\share\src\main.rs", "//server/share/src/main.rs"),
        ("//server/share/src/../tests", "//server/share/tests"),
        (r"\\server\\share\..\..\outside", "//server/share/outside"),
    ] {
        assert_eq!(
            EnvironmentPath::windows(input),
            Ok(canonical(expected)),
            "normalizing {input}"
        );
    }
}

#[test]
fn native_path_conversion_uses_the_requested_flavor() {
    for (path, posix, windows) in [
        ("/workspace/src", "/workspace/src", r"\workspace\src"),
        ("/c:/Users/Alice", "/c:/Users/Alice", r"c:\Users\Alice"),
        (
            "//server/share/src",
            "//server/share/src",
            r"\\server\share\src",
        ),
    ] {
        let path = EnvironmentPath::new(path).expect("valid canonical path");
        assert_eq!(
            path.to_native_path(PathFlavor::Posix),
            Ok(posix.to_string())
        );
        if path.as_str() == "/workspace/src" {
            assert_eq!(
                path.to_native_path(PathFlavor::Windows),
                Err(EnvironmentPathError::IncompatiblePathFlavor {
                    path: path.to_string(),
                    flavor: PathFlavor::Windows,
                })
            );
        } else {
            assert_eq!(
                path.to_native_path(PathFlavor::Windows),
                Ok(windows.to_string())
            );
        }
    }
}

#[test]
fn windows_conversion_normalizes_embedded_backslash_segments() {
    let path = EnvironmentPath::new(r"/c:/workspace/a\..\..\secret").expect("valid URI path");

    assert_eq!(
        path.to_native_path(PathFlavor::Windows),
        Ok(r"c:\secret".to_string())
    );
}

#[test]
fn windows_conversion_does_not_return_drive_relative_paths() {
    let path = EnvironmentPath::new("/workspace/src").expect("valid URI path");

    assert_eq!(
        path.to_native_path(PathFlavor::Windows),
        Err(EnvironmentPathError::IncompatiblePathFlavor {
            path: "/workspace/src".to_string(),
            flavor: PathFlavor::Windows,
        })
    );
}

#[test]
fn constructors_reject_empty_relative_and_null_paths() {
    for input in ["", "workspace/src", "C:relative", r"\rooted", "src\0file"] {
        assert!(
            EnvironmentPath::posix(input).is_err(),
            "accepting {input:?}"
        );
    }

    assert_eq!(
        EnvironmentPath::windows(""),
        Err(EnvironmentPathError::Empty)
    );
    assert_eq!(
        EnvironmentPath::windows("src\0file"),
        Err(EnvironmentPathError::ContainsNull)
    );
    for input in ["workspace/src", "C:relative", r"\rooted"] {
        assert_eq!(
            EnvironmentPath::windows(input),
            Err(EnvironmentPathError::NotAbsolute(input.to_string()))
        );
    }
}

#[test]
fn canonical_and_windows_constructors_reject_incomplete_unc_paths() {
    for input in ["//", "//server", "//server/"] {
        assert_eq!(
            EnvironmentPath::new(input),
            Err(EnvironmentPathError::InvalidWindowsUncPath(
                input.to_string()
            ))
        );
    }
    for input in [r"\\", r"\\server", r"\\server\"] {
        assert_eq!(
            EnvironmentPath::windows(input),
            Err(EnvironmentPathError::InvalidWindowsUncPath(
                input.to_string()
            ))
        );
    }
}

#[test]
fn windows_constructor_rejects_device_and_verbatim_namespaces() {
    for input in [
        r"\\?\C:\src",
        r"\\.\C:\src",
        r"\\?\UNC\server\share",
        r"\\.\PhysicalDrive0",
        "//?/C:/src",
        "//./pipe/name",
        r"\\?/C:\src",
        r"\\./pipe\name",
    ] {
        assert_eq!(
            EnvironmentPath::windows(input),
            Err(EnvironmentPathError::UnsupportedWindowsNamespace(
                input.to_string()
            ))
        );
    }
}

#[test]
fn canonical_constructor_rejects_windows_namespaces() {
    for input in ["//?/C:/src", "//./pipe/name", "//?/UNC/server/share"] {
        assert_eq!(
            EnvironmentPath::new(input),
            Err(EnvironmentPathError::UnsupportedWindowsNamespace(
                input.to_string()
            ))
        );
    }
}

#[test]
fn basename_handles_posix_drive_and_unc_roots() {
    for input in ["/", "/c:/", "//server/share"] {
        let path = EnvironmentPath::new(input).expect("valid root");
        assert_eq!(path.basename(), None, "basename for {input}");
    }
    for (input, expected) in [
        ("/workspace/src/lib.rs", "lib.rs"),
        ("/c:/Users/Alice", "Alice"),
        ("//server/share/src", "src"),
        ("/日本語/ファイル.rs", "ファイル.rs"),
    ] {
        let path = EnvironmentPath::new(input).expect("valid path");
        assert_eq!(path.basename(), Some(expected), "basename for {input}");
    }
}

#[test]
fn parent_stops_at_each_kind_of_root() {
    for (input, expected) in [
        ("/workspace/src/lib.rs", Some("/workspace/src")),
        ("/workspace", Some("/")),
        ("/", None),
        ("/c:/Users/Alice", Some("/c:/Users")),
        ("/c:/Users", Some("/c:/")),
        ("/c:/", None),
        ("//server/share/src", Some("//server/share")),
        ("//server/share", None),
    ] {
        let path = EnvironmentPath::new(input).expect("valid path");
        assert_eq!(path.parent(), expected.map(canonical), "parent for {input}");
    }
}

#[test]
fn join_normalizes_relative_segments_without_escaping_the_root() {
    for (base, relative, expected) in [
        (
            "/workspace/src",
            "../tests/test.rs",
            "/workspace/tests/test.rs",
        ),
        ("/", "../../etc", "/etc"),
        ("/c:/Users", "../../Windows", "/c:/Windows"),
        (
            "//server/share/src",
            "../../../outside",
            "//server/share/outside",
        ),
        ("/workspace", "", "/workspace"),
    ] {
        let base = EnvironmentPath::new(base).expect("valid base");
        assert_eq!(
            base.join(relative),
            Ok(canonical(expected)),
            "joining {relative}"
        );
    }
}

#[test]
fn join_rejects_absolute_and_null_paths() {
    let base = EnvironmentPath::new("/workspace").expect("valid base");
    assert_eq!(
        base.join("/src"),
        Err(EnvironmentPathError::JoinPathMustBeRelative(
            "/src".to_string()
        ))
    );
    assert_eq!(
        base.join("src\0file"),
        Err(EnvironmentPathError::ContainsNull)
    );
}

#[test]
fn lexical_identity_preserves_case_unicode_and_windows_specific_names() {
    let paths = [
        EnvironmentPath::new("/workspace/Foo").expect("valid path"),
        EnvironmentPath::new("/workspace/foo").expect("valid path"),
        EnvironmentPath::new("/workspace/\u{00e9}").expect("valid path"),
        EnvironmentPath::new("/workspace/e\u{301}").expect("valid path"),
        EnvironmentPath::new("/c:/CON.txt").expect("valid path"),
        EnvironmentPath::new("/c:/file:stream").expect("valid path"),
        EnvironmentPath::new("/c:/trailing. ").expect("valid path"),
    ];

    for (index, left) in paths.iter().enumerate() {
        for right in &paths[index + 1..] {
            assert_ne!(left, right);
        }
    }
}
