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
    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };
    assert_eq!(
        PathUri::parse(&uri_string).expect("serialized URI should parse"),
        uri
    );
    assert_eq!(
        view.path().to_native_path(PathFlavor::Posix),
        Ok(view.path().as_str().to_string())
    );
}

#[test]
fn file_uri_parses_a_windows_path_on_any_host() {
    let uri = PathUri::parse("file:///C:/Users/Alice%20Smith/src/main.rs")
        .expect("Windows file URI should parse on every host");

    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };
    assert_eq!(view.path().as_str(), "/C:/Users/Alice Smith/src/main.rs");
    assert_eq!(
        uri.to_string(),
        "file:///C:/Users/Alice%20Smith/src/main.rs"
    );
    assert_eq!(
        view.path().to_native_path(PathFlavor::Windows),
        Ok(r"c:\Users\Alice Smith\src\main.rs".to_string())
    );
}

#[test]
fn file_uri_parses_a_posix_path_on_any_host() {
    let uri = PathUri::parse("file:///home/alice/src/main.rs")
        .expect("POSIX file URI should parse on every host");

    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };
    assert_eq!(view.path().as_str(), "/home/alice/src/main.rs");
    assert_eq!(uri.to_string(), "file:///home/alice/src/main.rs");
}

#[test]
#[cfg(unix)]
fn file_uri_round_trips_posix_paths_that_resemble_windows_paths() {
    for input in ["/C:/Project", "/C:"] {
        let path = AbsolutePathBuf::from_absolute_path_checked(input).expect("absolute POSIX path");
        let uri = PathUri::from_file_path(&path).expect("path should convert to a file URI");
        let reparsed = PathUri::parse(&uri.to_string()).expect("file URI should reparse");
        let PathUriView::File(view) = uri.view() else {
            panic!("expected file view");
        };

        assert_eq!(view.path().as_str(), input);
        assert_eq!(reparsed, uri);
    }
}

#[test]
#[cfg(unix)]
fn file_uri_accepts_non_utf8_posix_paths() {
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec()));
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute POSIX path");

    let uri = PathUri::from_file_path(&path).expect("non-UTF-8 path should convert to a file URI");
    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };
    assert_eq!(
        view.to_native_path()
            .expect("URI should convert to native path"),
        path
    );
    assert_eq!(
        PathUri::parse(&uri.to_string()).expect("non-UTF-8 URI should reparse"),
        uri
    );
}

#[test]
#[cfg(unix)]
fn file_uri_round_trips_literal_percent_characters() {
    let path =
        AbsolutePathBuf::from_absolute_path_checked("/tmp/100%/file").expect("absolute POSIX path");
    let uri = PathUri::from_file_path(&path).expect("path should convert to a file URI");

    assert_eq!(uri.to_string(), "file:///tmp/100%25/file");
    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };
    assert_eq!(
        view.to_native_path()
            .expect("URI should convert to native path"),
        path
    );
}

#[test]
#[cfg(windows)]
fn file_uri_round_trips_windows_unc_paths() {
    let path = AbsolutePathBuf::from_absolute_path_checked(r"\\server\share\src\main.rs")
        .expect("absolute UNC path");
    let uri = PathUri::from_file_path(&path).expect("UNC path should convert to a file URI");
    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };

    assert_eq!(view.path().as_str(), "//server/share/src/main.rs");
}

#[test]
fn file_uri_path_view_retains_unc_authority() {
    let uri = PathUri::parse("file://server/share/src/main.rs").expect("valid file URI");
    let PathUriView::File(view) = uri.view() else {
        panic!("expected file view");
    };

    assert_eq!(view.path().as_str(), "//server/share/src/main.rs");
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
fn environment_uri_round_trips_a_unix_path() {
    let environment_id = EnvironmentId::new("dev_box-1").expect("valid environment id");
    let path = EnvironmentPath::posix("/workspace/a path/file.rs").expect("valid POSIX path");

    let uri = PathUri::from_environment_path(&environment_id, &path)
        .expect("path should convert to an environment URI");

    assert_eq!(
        uri.to_string(),
        "codex-env:///dev_box-1/workspace/a%20path/file.rs"
    );
    assert_eq!(
        uri.view(),
        PathUriView::Environment(EnvironmentUriView {
            environment_id,
            path,
        })
    );
}

#[test]
fn environment_uri_round_trips_a_windows_path_on_any_host() {
    let environment_id = EnvironmentId::new("windows-dev").expect("valid environment id");
    let path = EnvironmentPath::windows(r"C:\Users\Alice Smith\src\..\main.rs")
        .expect("valid Windows path");

    let uri = PathUri::from_environment_path(&environment_id, &path)
        .expect("path should convert to an environment URI");
    let reparsed = PathUri::parse(&uri.to_string()).expect("URI should parse");

    assert_eq!(
        uri.to_string(),
        "codex-env:///windows-dev/c:/Users/Alice%20Smith/main.rs"
    );
    assert_eq!(path.as_str(), "/c:/Users/Alice Smith/main.rs");
    assert_eq!(reparsed, uri);
    assert_eq!(
        uri.view(),
        PathUriView::Environment(EnvironmentUriView {
            environment_id,
            path,
        })
    );
}

#[test]
fn environment_uri_round_trips_a_windows_unc_path_on_any_host() {
    let environment_id = EnvironmentId::new("windows-dev").expect("valid environment id");
    let path =
        EnvironmentPath::windows(r"\\server\share\src\main.rs").expect("valid Windows UNC path");

    let uri = PathUri::from_environment_path(&environment_id, &path)
        .expect("path should convert to an environment URI");
    let reparsed = PathUri::parse(&uri.to_string()).expect("URI should parse");

    assert_eq!(
        uri.to_string(),
        "codex-env:///windows-dev//server/share/src/main.rs"
    );
    assert_eq!(path.as_str(), "//server/share/src/main.rs");
    assert_eq!(reparsed, uri);
}

#[test]
fn unknown_scheme_is_rejected_at_construction() {
    let error = PathUri::parse("artifact://store/object-1")
        .expect_err("unknown schemes should be rejected");

    assert!(matches!(
        error,
        PathUriParseError::UnsupportedScheme(scheme) if scheme == "artifact"
    ));
}

#[test]
fn path_uri_serializes_as_a_string() {
    let uri: PathUri = "codex-env:///devbox/workspace/src/lib.rs"
        .parse()
        .expect("valid environment URI");

    let json = serde_json::to_string(&uri).expect("URI should serialize");
    let deserialized: PathUri = serde_json::from_str(&json).expect("URI should deserialize");

    assert_eq!(json, r#""codex-env:///devbox/workspace/src/lib.rs""#);
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
fn parsed_uri_serializes_from_typed_components() {
    let uri = PathUri::parse("codex-env:///devbox/C:/workspace/./src/../lib.rs")
        .expect("valid environment URI");

    assert_eq!(uri.to_string(), "codex-env:///devbox/c:/workspace/lib.rs");
}

#[test]
fn environment_uri_dot_segments_cannot_change_the_environment_id() {
    for input in [
        "codex-env:///devbox/../prod/secret",
        "codex-env:///devbox/%2e%2e/prod/secret",
    ] {
        let uri = PathUri::parse(input).expect("environment URI should parse");
        let PathUriView::Environment(view) = uri.view() else {
            panic!("expected environment view");
        };

        assert_eq!(view.environment_id().as_str(), "devbox", "parsing {input}");
        assert_eq!(view.path().as_str(), "/prod/secret", "parsing {input}");
    }
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
fn environment_uri_rejects_authority_syntax() {
    let error = PathUri::parse("codex-env://devbox/workspace/file.rs")
        .expect_err("environment id must be a path segment");

    assert!(matches!(
        error,
        PathUriParseError::EnvironmentUriMustNotHaveAuthority
    ));
}

#[test]
fn environment_uri_rejects_missing_path() {
    let error =
        PathUri::parse("codex-env:///devbox").expect_err("environment URI should include a path");

    assert!(matches!(
        error,
        PathUriParseError::InvalidEnvironmentUriPath
    ));
}

#[test]
fn environment_uri_accepts_the_root_path() {
    let uri = PathUri::parse("codex-env:///devbox/").expect("root path should be valid");

    let PathUriView::Environment(view) = uri.view() else {
        panic!("expected environment view");
    };
    assert_eq!(view.environment_id().as_str(), "devbox");
    assert_eq!(view.path().as_str(), "/");
}

#[test]
fn known_path_uris_reject_queries_and_fragments() {
    let query_error =
        PathUri::parse("file:///tmp/file.rs?version=1").expect_err("query should be rejected");
    let fragment_error = PathUri::parse("codex-env:///devbox/tmp/file.rs#L1")
        .expect_err("fragment should be rejected");

    assert!(matches!(query_error, PathUriParseError::QueryNotAllowed));
    assert!(matches!(
        fragment_error,
        PathUriParseError::FragmentNotAllowed
    ));
}

#[test]
fn path_uris_reject_percent_encoded_path_separators() {
    for input in [
        "file:///tmp/a%2Fb",
        "file:///tmp/a%2fb",
        "codex-env:///devbox/tmp/a%2Fb",
        "codex-env:///devbox/tmp/a%2fb",
    ] {
        assert!(PathUri::parse(input).is_err(), "accepting {input}");
    }
}

#[test]
fn path_uris_reject_non_utf8_percent_encoding() {
    for input in [
        "file:///tmp/%00",
        "file:///tmp/%ZZ",
        "codex-env:///devbox/tmp/%F0%28%8C%28",
        "codex-env:///devbox/tmp/%00",
        "codex-env:///devbox/tmp/%",
    ] {
        assert!(PathUri::parse(input).is_err(), "accepting {input}");
    }
}

#[test]
fn encoded_filename_characters_round_trip_without_becoming_uri_metadata() {
    let uri = PathUri::parse("codex-env:///devbox/tmp/a%3Fb%23c%25d")
        .expect("encoded filename characters should parse");

    assert_eq!(uri.to_string(), "codex-env:///devbox/tmp/a%3Fb%23c%25d");
    let PathUriView::Environment(view) = uri.view() else {
        panic!("expected environment view");
    };
    assert_eq!(view.path().as_str(), "/tmp/a?b#c%d");
}

#[test]
fn double_encoded_separator_remains_filename_text() {
    let uri = PathUri::parse("codex-env:///devbox/tmp/a%252Fb")
        .expect("double-encoded separator should parse as filename text");

    assert_eq!(uri.to_string(), "codex-env:///devbox/tmp/a%252Fb");
    let PathUriView::Environment(view) = uri.view() else {
        panic!("expected environment view");
    };
    assert_eq!(view.path().as_str(), "/tmp/a%2Fb");
}

#[test]
fn environment_uri_round_trips_opaque_environment_ids() {
    let path = EnvironmentPath::posix("/workspace").expect("valid path");
    for id in [
        "dev_box-1",
        "local",
        "none",
        "dev.box",
        "日本語/environment",
        "a?b#c%d",
    ] {
        let environment_id = EnvironmentId::new(id).expect("valid opaque id");
        let uri = PathUri::from_environment_path(&environment_id, &path)
            .expect("opaque environment id should serialize");
        let reparsed = PathUri::parse(&uri.to_string()).expect("environment URI should reparse");
        let PathUriView::Environment(view) = reparsed.view() else {
            panic!("expected environment view");
        };

        assert_eq!(
            view.environment_id(),
            &environment_id,
            "round-tripping {id}"
        );
    }
}
