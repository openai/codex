use super::*;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use pretty_assertions::assert_eq;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

#[test]
fn file_uri_round_trips_an_absolute_path() {
    let path = AbsolutePathBuf::current_dir()
        .expect("current directory")
        .join("a path/file.rs");

    let uri = PathUri::from_abs_path(&path);

    let uri_string = uri.to_string();
    assert!(uri_string.starts_with("file:"));
    assert!(uri_string.ends_with("/a%20path/file.rs"));
    assert_eq!(
        PathUri::parse(&uri_string).expect("serialized URI should parse"),
        uri
    );
    assert_eq!(
        uri.to_abs_path()
            .expect("local file URI should convert to a native path"),
        path
    );
}

#[test]
fn non_native_uri_io_conversion_is_invalid_input() {
    #[cfg(unix)]
    let uri = PathUri::parse("file://server/share/file.txt").expect("valid file URI");
    #[cfg(windows)]
    let uri = PathUri::parse("file:///usr/local/file.txt").expect("valid file URI");

    let error = uri
        .to_abs_path()
        .expect_err("URI should not be host-native");

    assert_eq!(
        (error.kind(), error.to_string()),
        (
            io::ErrorKind::InvalidInput,
            format!("'{uri}' is invalid on '{}'", std::env::consts::OS),
        )
    );
}

#[test]
fn file_uri_parses_a_windows_path_on_any_host() {
    let uri = PathUri::parse("file:///C:/Users/Alice%20Smith/src/main.rs")
        .expect("Windows file URI should parse on every host");

    assert_eq!(uri.encoded_path(), "/C:/Users/Alice%20Smith/src/main.rs");
    assert_eq!(uri.basename(), Some("main.rs".to_string()));
    assert_eq!(
        uri.to_string(),
        "file:///C:/Users/Alice%20Smith/src/main.rs"
    );
}

#[test]
fn infers_path_conventions_from_uri_shape() {
    for (uri, expected) in [
        ("file:///", Some(PathConvention::Posix)),
        ("file:///home/alice/src", Some(PathConvention::Posix)),
        ("file:///C:/Users/Alice/src", Some(PathConvention::Windows)),
        ("file:///d:", Some(PathConvention::Windows)),
        ("file://server/share/src", Some(PathConvention::Windows)),
        // Opaque fallback for POSIX bytes `/tmp/null-\0-\xff-byte`.
        (
            "file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
            Some(PathConvention::Posix),
        ),
        // Opaque fallback for Windows UTF-16LE `\\.\COM1\`.
        (
            "file:///%00/bad/path/XABcAC4AXABDAE8ATQAxAFwA",
            Some(PathConvention::Windows),
        ),
        ("file:///%00/bad/path/YQ", None),
    ] {
        let path = PathUri::parse(uri).expect("valid path URI");

        assert_eq!(path.infer_path_convention(), expected, "inferring {uri}");
    }
}

#[test]
fn drive_shaped_posix_uri_is_intentionally_inferred_as_windows() {
    let path = PathUri::parse("file:///C:/actually/a/posix/path").expect("valid path URI");

    // `/C:/...` is valid on POSIX, but treating this uncommon spelling as a
    // Windows drive lets callers render the overwhelmingly more common foreign
    // Windows URI without separately carrying its source convention.
    assert_eq!(path.infer_path_convention(), Some(PathConvention::Windows));
}

#[test]
fn inferred_native_path_string_uses_the_inferred_convention() {
    for (uri, expected) in [
        ("file:///home/alice/a%20file.rs", "/home/alice/a file.rs"),
        (
            "file:///C:/Users/Alice%20Smith/main.rs",
            r"C:\Users\Alice Smith\main.rs",
        ),
        ("file://server/share/main.rs", r"\\server\share\main.rs"),
        ("file://server/", "file://server/"),
        ("file:///%00/bad/path/YQ", "file:///%00/bad/path/YQ"),
    ] {
        let path = PathUri::parse(uri).expect("valid path URI");

        assert_eq!(
            path.inferred_native_path_string(),
            expected,
            "rendering {uri}"
        );
        assert_eq!(
            LegacyAppPathString::from(path).as_str(),
            expected,
            "rendering typed API path {uri}"
        );
    }
}

#[cfg(windows)]
#[test]
fn file_uri_falls_back_for_windows_prefixes_without_a_uri_representation() {
    for (native_path, expected_uri) in [
        (r"\\.\COM1", "file:///%00/bad/path/XABcAC4AXABDAE8ATQAxAFwA"),
        (
            r"\\?\Volume{00000000-0000-0000-0000-000000000000}\file.rs",
            "file:///%00/bad/path/XABcAD8AXABWAG8AbAB1AG0AZQB7ADAAMAAwADAAMAAwADAAMAAtADAAMAAwADAALQAwADAAMAAwAC0AMAAwADAAMAAtADAAMAAwADAAMAAwADAAMAAwADAAMAAwAH0AXABmAGkAbABlAC4AcgBzAA",
        ),
    ] {
        let path = AbsolutePathBuf::from_absolute_path_checked(native_path)
            .expect("Windows namespace path should be absolute");

        let uri = PathUri::from_abs_path(&path);

        assert_eq!(uri.to_string(), expected_uri, "converting {native_path}");
        assert_eq!(
            PathUri::parse(&uri.to_string())
                .expect("fallback URI should parse")
                .to_abs_path()
                .expect("fallback URI should decode"),
            path,
            "round-tripping {native_path}"
        );
    }
}

#[cfg(windows)]
#[test]
fn file_uri_fallback_round_trips_non_unicode_windows_paths() {
    let path_wide = r"C:\bad\"
        .encode_utf16()
        .chain([0xd800])
        .collect::<Vec<_>>();
    let path = PathBuf::from(OsString::from_wide(&path_wide));
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute Windows path");

    let uri = PathUri::from_abs_path(&path);
    let reparsed = PathUri::parse(&uri.to_string()).expect("fallback URI should parse");

    assert!(uri.to_string().starts_with(BAD_PATH_URI_PREFIX));
    assert_eq!(
        reparsed.to_abs_path().expect("fallback URI should decode"),
        path
    );
}

#[cfg(unix)]
#[test]
fn file_uri_falls_back_for_posix_paths_with_null_bytes() {
    let path = PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/null-\0-\xff-byte".to_vec(),
    ));
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute POSIX path");

    let uri = PathUri::from_abs_path(&path);

    assert_eq!(
        uri,
        PathUri::parse("file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl")
            .expect("valid fallback URI")
    );
    let json = serde_json::to_string(&uri).expect("fallback URI should serialize");
    let reparsed: PathUri =
        serde_json::from_str(&json).expect("serialized fallback URI should parse");
    assert_eq!(json, r#""file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl""#);
    assert_eq!(reparsed, uri);
    assert_eq!(
        reparsed.to_abs_path().expect("fallback URI should decode"),
        path
    );
}

#[cfg(unix)]
#[test]
fn ordinary_bad_path_uri_is_not_decoded_as_a_fallback() {
    let path = AbsolutePathBuf::from_absolute_path_checked("/bad/path/L3RtcC9udWxsLQAt_y1ieXRl")
        .expect("absolute POSIX path");
    let uri = PathUri::from_abs_path(&path);

    assert_eq!(uri.to_string(), "file:///bad/path/L3RtcC9udWxsLQAt_y1ieXRl");
    assert_eq!(
        uri.to_abs_path().expect("URI should convert literally"),
        path
    );
}

#[test]
fn malformed_bad_path_uris_are_rejected() {
    for uri in [
        "file:///%00/bad/path/",
        "file:///%00/bad/path/not*base64",
        "file:///%00/bad/path/YQ==",
        "file:///%00/bad/path/YR",
        "file:///%00/bad/path/YQ/extra",
        "file:///%00/other/YQ",
    ] {
        assert_eq!(
            PathUri::parse(uri),
            Err(PathUriParseError::InvalidFileUriPath {
                path: uri.to_string(),
            }),
            "parsing {uri}"
        );
    }
}

#[test]
fn structurally_valid_bad_path_uri_with_invalid_native_payload_fails_conversion() {
    let uri = PathUri::parse("file:///%00/bad/path/YQ")
        .expect("canonical base64 fallback URI should parse");

    assert_eq!(
        uri.to_abs_path()
            .expect_err("relative fallback payload should not convert")
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn bad_path_uris_are_opaque_to_lexical_operations() {
    let uri = PathUri::parse("file:///%00/bad/path/YQ")
        .expect("canonical base64 fallback URI should parse");

    assert_eq!(uri.basename(), None);
    assert_eq!(uri.parent(), None);
    assert_eq!(uri.join(""), Ok(uri.clone()));
    assert_eq!(
        uri.join("child"),
        Err(PathUriParseError::InvalidFileUriPath {
            path: uri.to_string(),
        })
    );
}

#[test]
fn file_uri_parses_a_posix_path_on_any_host() {
    let uri = PathUri::parse("file:///home/alice/src/main.rs")
        .expect("POSIX file URI should parse on every host");

    assert_eq!(uri.encoded_path(), "/home/alice/src/main.rs");
    assert_eq!(uri.basename(), Some("main.rs".to_string()));
    assert_eq!(uri.to_string(), "file:///home/alice/src/main.rs");
}

#[test]
fn file_uri_preserves_paths_that_resemble_windows_paths() {
    for (input, expected_path) in [("file:///C:/Project", "/C:/Project"), ("file:///C:", "/C:")] {
        let uri = PathUri::parse(input).expect("file URI should parse");
        let reparsed = PathUri::parse(&uri.to_string()).expect("file URI should reparse");
        assert_eq!(uri.encoded_path(), expected_path);
        assert_eq!(reparsed, uri);
    }
}

#[test]
#[cfg(unix)]
fn file_uri_accepts_non_utf8_posix_paths() {
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec()));
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute POSIX path");

    let uri = PathUri::from_abs_path(&path);
    assert_eq!(
        uri.to_abs_path()
            .expect("URI should convert to native path"),
        path
    );
    assert_eq!(
        PathUri::parse(&uri.to_string()).expect("non-UTF-8 URI should reparse"),
        uri
    );
}

#[test]
fn file_uri_round_trips_literal_percent_characters() {
    let uri = PathUri::parse("file:///tmp/100%25/file").expect("file URI should parse");

    assert_eq!(uri.to_string(), "file:///tmp/100%25/file");
    assert_eq!(uri.encoded_path(), "/tmp/100%25/file");
    assert_eq!(uri.basename(), Some("file".to_string()));
}

#[test]
#[cfg(windows)]
fn file_uri_round_trips_windows_unc_paths() {
    let path = AbsolutePathBuf::from_absolute_path_checked(r"\\server\share\src\main.rs")
        .expect("absolute UNC path");
    let uri = PathUri::from_abs_path(&path);

    assert_eq!(uri.encoded_path(), "/share/src/main.rs");
    assert_eq!(uri.to_abs_path().expect("UNC URI should convert"), path);
}

#[test]
fn file_uri_retains_unc_authority() {
    let uri = PathUri::parse("file://server/share/src/main.rs").expect("valid file URI");

    assert_eq!(uri.encoded_path(), "/share/src/main.rs");
    assert_eq!(uri.to_string(), "file://server/share/src/main.rs");
}

#[test]
fn file_uri_spelling_aliases_have_one_canonical_form() {
    for input in [
        "FILE:///workspace/src",
        "file:/workspace/src",
        "file://localhost/workspace/src",
        "file://LOCALHOST/workspace/src",
    ] {
        let uri = PathUri::parse(input).expect("file URI alias should parse");
        assert_eq!(uri.to_string(), "file:///workspace/src", "parsing {input}");
    }
}

#[test]
fn unsupported_schemes_are_rejected_at_construction() {
    for (input, expected_scheme) in [
        ("codex-env:///devbox/workspace", "codex-env"),
        ("artifact://store/object-1", "artifact"),
        ("http://example.com/file", "http"),
        ("https://example.com/file", "https"),
        ("ssh://host/workspace", "ssh"),
        ("vscode-remote://ssh-remote+host/workspace", "vscode-remote"),
        ("untitled:Untitled-1", "untitled"),
    ] {
        let error = PathUri::parse(input).expect_err("unsupported schemes should be rejected");

        assert!(
            matches!(
                error,
                PathUriParseError::UnsupportedScheme(scheme) if scheme == expected_scheme
            ),
            "parsing {input}"
        );
    }
}

#[test]
fn path_uri_serializes_as_a_string() {
    let uri: PathUri = "file:///workspace/src/lib.rs"
        .parse()
        .expect("valid file URI");

    let json = serde_json::to_string(&uri).expect("URI should serialize");
    let deserialized: PathUri = serde_json::from_str(&json).expect("URI should deserialize");

    assert_eq!(json, r#""file:///workspace/src/lib.rs""#);
    assert_eq!(deserialized, uri);
}

#[test]
fn path_uri_deserializes_legacy_absolute_paths() {
    let path = AbsolutePathBuf::current_dir()
        .expect("current directory")
        .join("workspace/src");
    let json = serde_json::to_string(&path).expect("absolute path should serialize");
    let uri: PathUri = serde_json::from_str(&json).expect("legacy absolute path should parse");

    assert_eq!(uri, PathUri::from_abs_path(&path));
}

#[test]
fn path_uri_rejects_relative_native_paths() {
    let error = PathUri::from_path("src/lib.rs").expect_err("relative path should be rejected");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn path_uri_rejects_legacy_relative_paths_with_absolute_path_guard() {
    let base = AbsolutePathBuf::current_dir().expect("current directory");
    let _guard = AbsolutePathBufGuard::new(base.as_path());
    let error = serde_json::from_str::<PathUri>(r#""src/lib.rs""#)
        .expect_err("legacy relative path should be rejected");

    assert!(error.to_string().contains("path is not absolute"));
}

#[test]
fn unsupported_scheme_is_rejected_during_deserialization() {
    let error = serde_json::from_str::<PathUri>(r#""artifact://store/object-1""#)
        .expect_err("unsupported scheme should fail deserialization");

    assert!(
        error
            .to_string()
            .contains("unsupported path URI scheme `artifact`")
    );
}

#[test]
fn known_path_uris_reject_queries_and_fragments() {
    let query_error =
        PathUri::parse("file:///tmp/file.rs?version=1").expect_err("query should be rejected");
    let fragment_error =
        PathUri::parse("file:///tmp/file.rs#L1").expect_err("fragment should be rejected");

    assert!(matches!(query_error, PathUriParseError::QueryNotAllowed));
    assert!(matches!(
        fragment_error,
        PathUriParseError::FragmentNotAllowed
    ));
}

#[test]
fn path_uris_reject_encoded_null_bytes() {
    assert!(PathUri::parse("file:///tmp/%00").is_err());
}

#[test]
fn encoded_filename_characters_round_trip_without_becoming_uri_metadata() {
    let uri = PathUri::parse("file:///tmp/a%3Fb%23c%25d")
        .expect("encoded filename characters should parse");

    assert_eq!(uri.to_string(), "file:///tmp/a%3Fb%23c%25d");
    assert_eq!(uri.encoded_path(), "/tmp/a%3Fb%23c%25d");
    assert_eq!(uri.basename(), Some("a?b#c%d".to_string()));
}

#[test]
fn double_encoded_separator_remains_filename_text() {
    let uri = PathUri::parse("file:///tmp/a%252Fb")
        .expect("double-encoded separator should parse as filename text");

    assert_eq!(uri.to_string(), "file:///tmp/a%252Fb");
    assert_eq!(uri.encoded_path(), "/tmp/a%252Fb");
    assert_eq!(uri.basename(), Some("a%2Fb".to_string()));
}

#[test]
fn basename_uses_decoded_uri_segments() {
    for (input, expected) in [
        ("file:///", None),
        ("file:///workspace/src/lib.rs", Some("lib.rs")),
        ("file:///workspace/a%20file.rs", Some("a file.rs")),
        ("file:///C:/", Some("C:")),
        ("file://server/share", Some("share")),
    ] {
        let uri = PathUri::parse(input).expect("valid file URI");
        assert_eq!(
            uri.basename(),
            expected.map(str::to_string),
            "basename for {input}"
        );
    }
}

#[test]
fn parent_uses_uri_hierarchy_and_preserves_authority() {
    for (input, expected) in [
        (
            "file:///workspace/src/lib.rs",
            Some("file:///workspace/src"),
        ),
        ("file:///workspace", Some("file:///")),
        ("file:///", None),
        ("file:///C:/Users", Some("file:///C:")),
        ("file:///C:/", Some("file:///")),
        (
            "file://server/share/src/main.rs",
            Some("file://server/share/src"),
        ),
        ("file://server/share", Some("file://server/")),
    ] {
        let uri = PathUri::parse(input).expect("valid file URI");
        let expected = expected.map(|value| PathUri::parse(value).expect("valid expected URI"));
        assert_eq!(uri.parent(), expected, "parent for {input}");
    }
}

#[test]
fn join_normalizes_relative_uri_segments() {
    for (base, relative, expected) in [
        (
            "file:///workspace/src",
            "../tests/test.rs",
            "file:///workspace/tests/test.rs",
        ),
        ("file:///", "../../etc", "file:///etc"),
        ("file:///C:/Users", "../Windows", "file:///C:/Windows"),
        (
            "file://server/share/src",
            "../tests",
            "file://server/share/tests",
        ),
        (
            "file:///workspace",
            "a?b#c%d",
            "file:///workspace/a%3Fb%23c%25d",
        ),
        ("file:///workspace/", "", "file:///workspace/"),
    ] {
        let base = PathUri::parse(base).expect("valid base URI");
        let expected = PathUri::parse(expected).expect("valid expected URI");
        assert_eq!(base.join(relative), Ok(expected), "joining {relative}");
    }
}

#[test]
fn join_replaces_posix_absolute_path() {
    let base = PathUri::parse("file:///workspace").expect("valid base URI");

    assert_eq!(
        base.join("/src"),
        Ok(PathUri::parse("file:///src").expect("valid absolute URI"))
    );
}

#[test]
fn join_replaces_windows_absolute_path() {
    let base = PathUri::parse("file:///C:/workspace/src").expect("valid base URI");

    assert_eq!(
        base.join(r"D:\tmp\test.rs"),
        Ok(PathUri::parse("file:///D:/tmp/test.rs").expect("valid absolute URI"))
    );
}

#[test]
fn join_windows_root_relative_path_preserves_drive_or_share() {
    for (base, path, expected) in [
        ("file:///C:/base/dir", r"\Windows", "file:///C:/Windows"),
        (
            "file://server/share/base/dir",
            r"\Windows",
            "file://server/share/Windows",
        ),
    ] {
        let base = PathUri::parse(base).expect("valid base URI");
        let expected = PathUri::parse(expected).expect("valid expected URI");
        assert_eq!(base.join(path), Ok(expected), "joining {path}");
    }
}

#[test]
fn join_rejects_windows_drive_relative_path() {
    let base = PathUri::parse("file:///C:/base").expect("valid base URI");

    assert_eq!(
        base.join(r"D:tmp"),
        Err(PathUriParseError::InvalidFileUriPath {
            path: r"D:tmp".to_string(),
        })
    );
}

#[test]
fn join_parent_segments_preserve_windows_drive_or_share_anchor() {
    for (base, expected) in [
        ("file:///C:/base/dir", "file:///C:/Windows"),
        (
            "file://server/share/base/dir",
            "file://server/share/Windows",
        ),
    ] {
        let base = PathUri::parse(base).expect("valid base URI");
        let expected = PathUri::parse(expected).expect("valid expected URI");
        assert_eq!(base.join(r"..\..\..\Windows"), Ok(expected));
    }
}

#[test]
fn join_rejects_null_paths() {
    let base = PathUri::parse("file:///workspace").expect("valid base URI");

    assert_eq!(
        base.join("src\0file"),
        Err(PathUriParseError::InvalidFileUriPath {
            path: "src\0file".to_string(),
        })
    );
}

#[test]
fn join_uses_the_base_uri_path_convention() {
    for (base, path, expected) in [
        (
            "file:///workspace/src",
            "../tests/test.rs",
            "file:///workspace/tests/test.rs",
        ),
        (
            "file:///C:/workspace/src",
            r"..\tests\test.rs",
            "file:///C:/workspace/tests/test.rs",
        ),
    ] {
        let base = PathUri::parse(base).expect("valid base URI");
        let expected = PathUri::parse(expected).expect("valid expected URI");
        assert_eq!(base.join(path), Ok(expected), "joining {path}");
    }
}

#[test]
fn to_url_returns_the_validated_url() {
    let uri = PathUri::parse("file://localhost/workspace/a%20file.rs").expect("valid file URI");

    assert_eq!(
        uri.to_url(),
        Url::parse("file:///workspace/a%20file.rs").expect("valid URL")
    );
}
