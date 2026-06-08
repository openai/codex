use super::*;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LegacyFilePathField {
    #[serde(with = "legacy_file_path_serde")]
    path: PathUri,
}

#[test]
fn file_uri_round_trips_an_absolute_path() {
    let path = AbsolutePathBuf::current_dir()
        .expect("current directory")
        .join("a path/file.rs");

    let uri = PathUri::from_file_path(&path).expect("path should convert to a file URI");

    let uri_string = uri.to_string();
    assert!(uri_string.starts_with("file:"));
    assert!(uri_string.ends_with("/a%20path/file.rs"));
    assert_eq!(
        PathUri::parse(&uri_string).expect("serialized URI should parse"),
        uri
    );
    assert_eq!(
        uri.to_native_path()
            .expect("local file URI should convert to a native path"),
        path
    );
}

#[test]
fn file_uri_parses_a_windows_path_on_any_host() {
    let uri = PathUri::parse("file:///C:/Users/Alice%20Smith/src/main.rs")
        .expect("Windows file URI should parse on every host");

    assert_eq!(uri.path(), "/C:/Users/Alice%20Smith/src/main.rs");
    assert_eq!(uri.basename(), Some("main.rs".to_string()));
    assert_eq!(
        uri.to_string(),
        "file:///C:/Users/Alice%20Smith/src/main.rs"
    );
}

#[test]
fn file_uri_parses_a_posix_path_on_any_host() {
    let uri = PathUri::parse("file:///home/alice/src/main.rs")
        .expect("POSIX file URI should parse on every host");

    assert_eq!(uri.path(), "/home/alice/src/main.rs");
    assert_eq!(uri.basename(), Some("main.rs".to_string()));
    assert_eq!(uri.to_string(), "file:///home/alice/src/main.rs");
}

#[test]
fn file_uri_preserves_paths_that_resemble_windows_paths() {
    for (input, expected_path) in [("file:///C:/Project", "/C:/Project"), ("file:///C:", "/C:")] {
        let uri = PathUri::parse(input).expect("file URI should parse");
        let reparsed = PathUri::parse(&uri.to_string()).expect("file URI should reparse");
        assert_eq!(uri.path(), expected_path);
        assert_eq!(reparsed, uri);
    }
}

#[test]
#[cfg(unix)]
fn file_uri_accepts_non_utf8_posix_paths() {
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec()));
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute POSIX path");

    let uri = PathUri::from_file_path(&path).expect("non-UTF-8 path should convert to a file URI");
    assert_eq!(
        uri.to_native_path()
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
    assert_eq!(uri.path(), "/tmp/100%25/file");
    assert_eq!(uri.basename(), Some("file".to_string()));
}

#[test]
#[cfg(windows)]
fn file_uri_round_trips_windows_unc_paths() {
    let path = AbsolutePathBuf::from_absolute_path_checked(r"\\server\share\src\main.rs")
        .expect("absolute UNC path");
    let uri = PathUri::from_file_path(&path).expect("UNC path should convert to a file URI");

    assert_eq!(uri.path(), "/share/src/main.rs");
    assert_eq!(uri.to_native_path().expect("UNC URI should convert"), path);
}

#[test]
fn file_uri_retains_unc_authority() {
    let uri = PathUri::parse("file://server/share/src/main.rs").expect("valid file URI");

    assert_eq!(uri.path(), "/share/src/main.rs");
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

    assert_eq!(
        uri,
        PathUri::from_file_path(&path).expect("expected file URI")
    );
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
fn legacy_file_path_serde_preserves_the_existing_wire_format() {
    let base = AbsolutePathBuf::current_dir().expect("current directory");
    let uri = PathUri::from_file_path(&base.join("src/lib.rs")).expect("file URI");
    let field = LegacyFilePathField { path: uri };

    let json = serde_json::to_string(&field).expect("legacy field should serialize");
    let _guard = AbsolutePathBufGuard::new(base.as_path());
    let reparsed: LegacyFilePathField =
        serde_json::from_str(&json).expect("legacy field should deserialize");

    assert_eq!(reparsed, field);
    assert!(!json.contains("file:"));
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
fn path_uris_reject_percent_encoded_path_separators() {
    for input in ["file:///tmp/a%2Fb", "file:///tmp/a%2fb"] {
        assert!(PathUri::parse(input).is_err(), "accepting {input}");
    }
}

#[test]
fn path_uris_reject_non_utf8_percent_encoding() {
    for input in ["file:///tmp/%00", "file:///tmp/%ZZ", "file:///tmp/%"] {
        assert!(PathUri::parse(input).is_err(), "accepting {input}");
    }
}

#[test]
fn encoded_filename_characters_round_trip_without_becoming_uri_metadata() {
    let uri = PathUri::parse("file:///tmp/a%3Fb%23c%25d")
        .expect("encoded filename characters should parse");

    assert_eq!(uri.to_string(), "file:///tmp/a%3Fb%23c%25d");
    assert_eq!(uri.path(), "/tmp/a%3Fb%23c%25d");
    assert_eq!(uri.basename(), Some("a?b#c%d".to_string()));
}

#[test]
fn double_encoded_separator_remains_filename_text() {
    let uri = PathUri::parse("file:///tmp/a%252Fb")
        .expect("double-encoded separator should parse as filename text");

    assert_eq!(uri.to_string(), "file:///tmp/a%252Fb");
    assert_eq!(uri.path(), "/tmp/a%252Fb");
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
fn join_rejects_absolute_and_null_paths() {
    let base = PathUri::parse("file:///workspace").expect("valid base URI");

    assert!(matches!(
        base.join("/src"),
        Err(PathUriParseError::JoinPathMustBeRelative(path)) if path == "/src"
    ));
    assert!(matches!(
        base.join("src\0file"),
        Err(PathUriParseError::InvalidFileUriPath)
    ));
}

#[test]
fn to_url_returns_the_validated_url() {
    let uri = PathUri::parse("file://localhost/workspace/a%20file.rs").expect("valid file URI");

    assert_eq!(
        uri.to_url(),
        Url::parse("file:///workspace/a%20file.rs").expect("valid URL")
    );
}
