use super::*;
use crate::PathUri;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy, Debug)]
struct RenderCase {
    uri: &'static str,
    convention: PathConvention,
    expected: RenderExpectation,
}

impl RenderCase {
    const fn round_trips(
        uri: &'static str,
        convention: PathConvention,
        rendered: &'static str,
    ) -> Self {
        Self {
            uri,
            convention,
            expected: RenderExpectation::RoundTrip(rendered),
        }
    }

    const fn rejects(uri: &'static str, convention: PathConvention, error: ExpectedError) -> Self {
        Self {
            uri,
            convention,
            expected: RenderExpectation::Error(error),
        }
    }

    const fn renders_lossily(
        uri: &'static str,
        convention: PathConvention,
        rendered: &'static str,
    ) -> Self {
        Self {
            uri,
            convention,
            expected: RenderExpectation::RenderOnly(rendered),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RenderExpectation {
    RoundTrip(&'static str),
    RenderOnly(&'static str),
    Error(ExpectedError),
}

#[derive(Clone, Copy, Debug)]
enum ExpectedError {
    OpaqueFallback,
    IncompatibleConvention,
}

const RENDER_CASES: &[RenderCase] = &[
    // POSIX paths.
    RenderCase::round_trips("file:///", PathConvention::Posix, "/"),
    RenderCase::round_trips(
        "file:///home/alice/src/main.rs",
        PathConvention::Posix,
        "/home/alice/src/main.rs",
    ),
    RenderCase::round_trips(
        "file:///home/alice/a%20file.rs",
        PathConvention::Posix,
        "/home/alice/a file.rs",
    ),
    RenderCase::round_trips(
        "file:///workspace/src/lib.rs",
        PathConvention::Posix,
        "/workspace/src/lib.rs",
    ),
    RenderCase::round_trips(
        "file:///workspace/tests/test.rs",
        PathConvention::Posix,
        "/workspace/tests/test.rs",
    ),
    RenderCase::round_trips("file:///etc", PathConvention::Posix, "/etc"),
    RenderCase::round_trips("file:///tmp/", PathConvention::Posix, "/tmp/"),
    RenderCase::round_trips("file:///C:/Project", PathConvention::Posix, "/C:/Project"),
    RenderCase::round_trips("file:///C:", PathConvention::Posix, "/C:"),
    RenderCase::round_trips("file:///tmp/%E2%98%83", PathConvention::Posix, "/tmp/☃"),
    RenderCase::round_trips("file:///tmp/a%5Cb", PathConvention::Posix, "/tmp/a\\b"),
    RenderCase::round_trips(
        "file:///tmp/100%25/file",
        PathConvention::Posix,
        "/tmp/100%/file",
    ),
    RenderCase::round_trips(
        "file:///tmp/a%3Fb%23c%25d",
        PathConvention::Posix,
        "/tmp/a?b#c%d",
    ),
    RenderCase::round_trips("file:///tmp/a%252Fb", PathConvention::Posix, "/tmp/a%2Fb"),
    RenderCase::round_trips(
        "file:///bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        PathConvention::Posix,
        "/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
    ),
    RenderCase::round_trips(
        "FILE:///workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::round_trips(
        "file:/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::round_trips(
        "file://localhost/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::round_trips(
        "file://LOCALHOST/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    // Windows drive paths.
    RenderCase::round_trips(
        "file:///C:/Users/Alice%20Smith/src/main.rs",
        PathConvention::Windows,
        r"C:\Users\Alice Smith\src\main.rs",
    ),
    RenderCase::round_trips("file:///C:/", PathConvention::Windows, "C:\\"),
    RenderCase::renders_lossily("file:///C:", PathConvention::Windows, "C:\\"),
    RenderCase::round_trips("file:///C:/Users", PathConvention::Windows, r"C:\Users"),
    RenderCase::round_trips("file:///C:/Windows", PathConvention::Windows, r"C:\Windows"),
    RenderCase::round_trips(
        "file:///d:/snowman/%E2%98%83",
        PathConvention::Windows,
        r"D:\snowman\☃",
    ),
    RenderCase::round_trips("file:///C:/tmp/", PathConvention::Windows, "C:\\tmp\\"),
    RenderCase::round_trips(
        "file:///C:/test%20with%20%25/path",
        PathConvention::Windows,
        r"C:\test with %\path",
    ),
    RenderCase::round_trips(
        "file:///C:/test%20with%20%2525/c%23code",
        PathConvention::Windows,
        r"C:\test with %25\c#code",
    ),
    RenderCase::round_trips(
        "file:///C:/Source/Z%C3%BCrich%20or%20Zurich%20(%CB%88zj%CA%8A%C9%99r%C9%AAk,/Code/resources/app/plugins/c%23/plugin.json",
        PathConvention::Windows,
        r"C:\Source\Zürich or Zurich (ˈzjʊərɪk,\Code\resources\app\plugins\c#\plugin.json",
    ),
    RenderCase::round_trips(
        "file:///C:/project/owner's_file/database.sqlite",
        PathConvention::Windows,
        r"C:\project\owner's_file\database.sqlite",
    ),
    RenderCase::round_trips(
        "file:///C:/project/%25A0.txt",
        PathConvention::Windows,
        r"C:\project\%A0.txt",
    ),
    RenderCase::round_trips(
        "file:///C:/project/%252e.txt",
        PathConvention::Windows,
        r"C:\project\%2e.txt",
    ),
    // Windows UNC paths.
    RenderCase::round_trips(
        "file://server/share/src/main.rs",
        PathConvention::Windows,
        r"\\server\share\src\main.rs",
    ),
    RenderCase::round_trips(
        "file://server/share",
        PathConvention::Windows,
        r"\\server\share",
    ),
    RenderCase::round_trips(
        "file://server/share/",
        PathConvention::Windows,
        "\\\\server\\share\\",
    ),
    RenderCase::round_trips(
        "file://shares/files/c%23/p.cs",
        PathConvention::Windows,
        r"\\shares\files\c#\p.cs",
    ),
    RenderCase::round_trips(
        "file://monacotools1/certificates/SSL/",
        PathConvention::Windows,
        "\\\\monacotools1\\certificates\\SSL\\",
    ),
    // Opaque fallbacks rendered according to their source convention.
    RenderCase::renders_lossily(
        "file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        PathConvention::Posix,
        "/tmp/null-\0-�-byte",
    ),
    RenderCase::round_trips(
        "file:///%00/bad/path/XABcAC4AXABDAE8ATQAxAFwA",
        PathConvention::Windows,
        r"\\.\COM1\",
    ),
    RenderCase::round_trips(
        "file:///%00/bad/path/XABcAD8AXABWAG8AbAB1AG0AZQB7ADAAMAAwADAAMAAwADAAMAAtADAAMAAwADAALQAwADAAMAAwAC0AMAAwADAAMAAtADAAMAAwADAAMAAwADAAMAAwADAAMAAwAH0AXABmAGkAbABlAC4AcgBzAA",
        PathConvention::Windows,
        r"\\?\Volume{00000000-0000-0000-0000-000000000000}\file.rs",
    ),
    // Windows rendering preserves path text without filesystem validation.
    RenderCase::round_trips("file:///C:/a%3Fb", PathConvention::Windows, "C:\\a?b"),
    RenderCase::round_trips("file:///C:/a*b", PathConvention::Windows, "C:\\a*b"),
    RenderCase::round_trips(
        "file:///C:/trailing.",
        PathConvention::Windows,
        "C:\\trailing.",
    ),
    RenderCase::round_trips(
        "file:///C:/trailing%20",
        PathConvention::Windows,
        "C:\\trailing ",
    ),
    RenderCase::round_trips(
        "file:///C:/control-%01",
        PathConvention::Windows,
        "C:\\control-\u{1}",
    ),
    RenderCase::round_trips(
        "file:///C:/file.txt:stream",
        PathConvention::Windows,
        "C:\\file.txt:stream",
    ),
    RenderCase::round_trips(
        "file://server/sh%3Fare/file.rs",
        PathConvention::Windows,
        "\\\\server\\sh?are\\file.rs",
    ),
    // These renderings intentionally lose URI byte or segment boundaries.
    RenderCase::renders_lossily(
        "file:///tmp/non-utf8-%FF",
        PathConvention::Posix,
        "/tmp/non-utf8-�",
    ),
    RenderCase::renders_lossily(
        "file:///tmp/non-utf8-%A0",
        PathConvention::Posix,
        "/tmp/non-utf8-�",
    ),
    RenderCase::renders_lossily("file:///tmp/a%2Fb", PathConvention::Posix, "/tmp/a/b"),
    RenderCase::renders_lossily("file:///C:/a%2Fb", PathConvention::Windows, "C:\\a/b"),
    RenderCase::renders_lossily("file:///C:/a%5Cb", PathConvention::Windows, "C:\\a\\b"),
    // URI shapes that do not match the requested convention.
    RenderCase::rejects(
        "file://server/share/file.txt",
        PathConvention::Posix,
        ExpectedError::IncompatibleConvention,
    ),
    RenderCase::rejects(
        "file://server/share/file.rs",
        PathConvention::Posix,
        ExpectedError::IncompatibleConvention,
    ),
    RenderCase::rejects(
        "file:///usr/local/file.txt",
        PathConvention::Windows,
        ExpectedError::IncompatibleConvention,
    ),
    RenderCase::rejects(
        "file:///home/alice/file.rs",
        PathConvention::Windows,
        ExpectedError::IncompatibleConvention,
    ),
    RenderCase::rejects(
        "file://server/",
        PathConvention::Windows,
        ExpectedError::IncompatibleConvention,
    ),
    RenderCase::rejects(
        "file:///_:/path",
        PathConvention::Windows,
        ExpectedError::IncompatibleConvention,
    ),
    // Invalid opaque fallback payloads.
    RenderCase::rejects(
        "file:///%00/bad/path/YQ",
        PathConvention::Posix,
        ExpectedError::OpaqueFallback,
    ),
    RenderCase::rejects(
        "file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        PathConvention::Windows,
        ExpectedError::OpaqueFallback,
    ),
];

#[test]
fn renders_native_paths_from_shared_cases() {
    for case in RENDER_CASES {
        let path = PathUri::parse(case.uri).expect("valid file URI");
        let expected = match case.expected {
            RenderExpectation::RoundTrip(rendered) => Ok(LegacyAppPathString(rendered.to_string())),
            RenderExpectation::RenderOnly(rendered) => {
                Ok(LegacyAppPathString(rendered.to_string()))
            }
            RenderExpectation::Error(ExpectedError::OpaqueFallback) => {
                Err(LegacyAppPathStringError::OpaqueFallback {
                    path: path.to_string(),
                })
            }
            RenderExpectation::Error(ExpectedError::IncompatibleConvention) => {
                Err(LegacyAppPathStringError::IncompatibleConvention {
                    path: path.to_string(),
                    convention: case.convention,
                })
            }
        };
        let actual = LegacyAppPathString::from_path_uri(&path, case.convention);

        assert_eq!(actual, expected, "rendering {case:?}");
        if let Ok(rendered) = &actual {
            assert_eq!(
                rendered.infer_absolute_path_convention(),
                Some(case.convention),
                "inferring {case:?}"
            );
        }

        if let RenderExpectation::RoundTrip(rendered) = case.expected {
            let api_path =
                serde_json::from_value::<LegacyAppPathString>(serde_json::json!(rendered))
                    .expect("native path should deserialize from API text");
            let reparsed = api_path
                .to_path_uri(case.convention)
                .expect("native path should parse using its convention");
            assert_eq!(reparsed, path, "parsing {case:?}");
            assert_eq!(
                LegacyAppPathString::from_path_uri(&reparsed, case.convention),
                Ok(api_path),
                "round-tripping {case:?}"
            );
        }
    }
}

#[test]
fn relative_api_path_serializes_and_deserializes_unchanged() {
    for raw_path in [".", "subdir", "subdir/file.rs"] {
        let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
            .expect("relative API path should deserialize");

        assert_eq!(
            serde_json::to_value(path).expect("relative API path should serialize"),
            serde_json::json!(raw_path)
        );
    }
}

#[test]
fn relative_api_path_is_invalid_when_converted_to_a_path_uri() {
    let raw_path = "subdir";
    let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
        .expect("relative API path should deserialize");

    assert_eq!(path.infer_absolute_path_convention(), None);
    assert_eq!(
        path.to_path_uri(PathConvention::Posix),
        Err(LegacyAppPathStringError::InvalidNativePath {
            path: raw_path.to_string(),
            convention: Some(PathConvention::Posix),
        })
    );
    assert_eq!(
        PathUri::try_from(path.clone()),
        Err(LegacyAppPathStringError::InvalidNativePath {
            path: raw_path.to_string(),
            convention: None,
        })
    );
    assert_eq!(
        AbsolutePathBuf::try_from(path),
        Err(LegacyAppPathStringError::InvalidNativePath {
            path: raw_path.to_string(),
            convention: None,
        })
    );
}

#[test]
fn other_non_absolute_api_paths_cannot_be_converted_to_path_uris() {
    for (raw_path, convention) in [
        (r"workspace\file.rs", PathConvention::Windows),
        (r"C:file.rs", PathConvention::Windows),
    ] {
        let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
            .expect("API path should deserialize without validation");

        assert_eq!(path.infer_absolute_path_convention(), None);
        assert_eq!(
            path.to_path_uri(convention),
            Err(LegacyAppPathStringError::InvalidNativePath {
                path: raw_path.to_string(),
                convention: Some(convention),
            })
        );
    }
}

#[test]
fn infers_absolute_path_conventions_from_api_text() {
    for (raw_path, expected) in [
        (r"C:\workspace\file.rs", Some(PathConvention::Windows)),
        ("c:/workspace/file.rs", Some(PathConvention::Windows)),
        (r"\\server\share\file.rs", Some(PathConvention::Windows)),
        (r"\\?\C:\workspace\file.rs", Some(PathConvention::Windows)),
        (r"\\.\COM1", Some(PathConvention::Windows)),
        ("/workspace/file.rs", Some(PathConvention::Posix)),
        ("/C:/workspace/file.rs", Some(PathConvention::Posix)),
        ("//server/share/file.rs", Some(PathConvention::Posix)),
        ("", None),
        (".", None),
        ("subdir/file.rs", None),
        (r"subdir\file.rs", None),
        (r"C:file.rs", None),
        (r"\rooted-without-drive", None),
    ] {
        let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
            .expect("API path should deserialize without validation");

        assert_eq!(
            path.infer_absolute_path_convention(),
            expected,
            "inferring {raw_path:?}"
        );
    }
}

#[test]
fn converts_absolute_api_paths_using_the_inferred_convention() {
    for (raw_path, convention, expected_uri) in [
        (
            r"C:\workspace\file.rs",
            PathConvention::Windows,
            "file:///C:/workspace/file.rs",
        ),
        (
            "/workspace/file.rs",
            PathConvention::Posix,
            "file:///workspace/file.rs",
        ),
    ] {
        let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
            .expect("absolute API path should deserialize");

        assert_eq!(
            path.to_inferred_path_uri(),
            Some(PathUri::parse(expected_uri).expect("expected URI should parse")),
        );
        assert_eq!(path.render_for_ui(), raw_path);
        assert_eq!(
            PathUri::try_from(path.clone()),
            path.to_path_uri(convention)
        );
    }
}

#[test]
fn converts_native_api_path_to_inferred_absolute_path() {
    #[cfg(windows)]
    let raw_path = r"C:\workspace\file.rs";
    #[cfg(not(windows))]
    let raw_path = "/workspace/file.rs";
    let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
        .expect("absolute API path should deserialize");
    let expected = AbsolutePathBuf::try_from(raw_path).expect("native absolute path should parse");

    assert_eq!(
        AbsolutePathBuf::try_from(path.clone()),
        Ok(expected.clone())
    );
    assert_eq!(path.to_inferred_abs_path(), Some(expected));
}

#[test]
fn foreign_absolute_syntax_deserializes_without_host_interpretation() {
    for (raw_path, convention) in [
        (r"C:\workspace\file.rs", PathConvention::Windows),
        ("/workspace/file.rs", PathConvention::Posix),
    ] {
        let path = serde_json::from_value::<LegacyAppPathString>(serde_json::json!(raw_path))
            .expect("foreign API path should deserialize");

        assert_eq!(path.as_str(), raw_path);
        assert_eq!(path.infer_absolute_path_convention(), Some(convention));
    }
}

#[test]
fn from_path_preserves_foreign_absolute_path_for_uri_conversion() {
    #[cfg(not(windows))]
    let (foreign_path, expected_uri) = (r"C:\Users\openai\share", "file:///C:/Users/openai/share");
    #[cfg(windows)]
    let (foreign_path, expected_uri) = ("/home/openai/share", "file:///home/openai/share");

    let path: PathUri = LegacyAppPathString::from_path(std::path::Path::new(foreign_path))
        .try_into()
        .expect("foreign absolute path should convert");

    assert_eq!(
        path,
        PathUri::parse(expected_uri).expect("valid expected URI")
    );
}

#[test]
fn renders_an_absolute_path_using_the_host_convention() {
    #[cfg(unix)]
    let native_path = "/workspace/a file.rs";
    #[cfg(windows)]
    let native_path = r"C:\workspace\a file.rs";
    let path = AbsolutePathBuf::from_absolute_path_checked(native_path)
        .expect("native path should be absolute");

    assert_eq!(
        LegacyAppPathString::from(path),
        LegacyAppPathString(native_path.to_string())
    );
}

#[cfg(windows)]
#[test]
fn renders_native_non_unicode_windows_fallback_lossily() {
    use std::os::windows::ffi::OsStringExt;

    let native_path = std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &r"C:\bad\"
            .encode_utf16()
            .chain([0xd800])
            .collect::<Vec<_>>(),
    ));
    let native_path =
        AbsolutePathBuf::from_absolute_path_checked(native_path).expect("absolute native path");

    assert_eq!(
        LegacyAppPathString::from_abs_path(&native_path),
        LegacyAppPathString(r"C:\bad\�".to_string())
    );

    let path = PathUri::from_abs_path(&native_path);

    assert_eq!(
        LegacyAppPathString::from_path_uri(&path, PathConvention::Windows),
        Ok(LegacyAppPathString(r"C:\bad\�".to_string()))
    );
    assert_eq!(
        LegacyAppPathString::from_path_uri(&path, PathConvention::Posix),
        Err(LegacyAppPathStringError::OpaqueFallback {
            path: path.to_string(),
        })
    );
}

#[test]
fn serializes_and_deserializes_as_a_string() {
    let path = PathUri::parse("file:///workspace/src/lib.rs").expect("valid file URI");
    let rendered = LegacyAppPathString::from_path_uri(&path, PathConvention::Posix)
        .expect("POSIX URI should render");

    let json = serde_json::to_string(&rendered).expect("rendered path should serialize");
    assert_eq!(json, r#""/workspace/src/lib.rs""#);
    assert_eq!(
        serde_json::from_str::<LegacyAppPathString>(&json)
            .expect("rendered path should deserialize from a string"),
        rendered
    );
}
