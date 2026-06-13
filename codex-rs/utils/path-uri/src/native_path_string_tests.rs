use super::*;
use crate::PathUri;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy, Debug)]
struct RenderCase {
    uri: &'static str,
    convention: PathConvention,
    expected: RenderExpectation,
}

impl RenderCase {
    const fn renders(
        uri: &'static str,
        convention: PathConvention,
        rendered: &'static str,
    ) -> Self {
        Self {
            uri,
            convention,
            expected: RenderExpectation::Rendered(rendered),
        }
    }

    const fn rejects(uri: &'static str, convention: PathConvention, error: ExpectedError) -> Self {
        Self {
            uri,
            convention,
            expected: RenderExpectation::Error(error),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RenderExpectation {
    Rendered(&'static str),
    Error(ExpectedError),
}

#[derive(Clone, Copy, Debug)]
enum ExpectedError {
    OpaqueFallback,
    IncompatibleConvention,
    NonUtf8,
    EncodedSeparator,
}

const RENDER_CASES: &[RenderCase] = &[
    // POSIX paths.
    RenderCase::renders("file:///", PathConvention::Posix, "/"),
    RenderCase::renders(
        "file:///home/alice/src/main.rs",
        PathConvention::Posix,
        "/home/alice/src/main.rs",
    ),
    RenderCase::renders(
        "file:///home/alice/a%20file.rs",
        PathConvention::Posix,
        "/home/alice/a file.rs",
    ),
    RenderCase::renders(
        "file:///workspace/src/lib.rs",
        PathConvention::Posix,
        "/workspace/src/lib.rs",
    ),
    RenderCase::renders(
        "file:///workspace/tests/test.rs",
        PathConvention::Posix,
        "/workspace/tests/test.rs",
    ),
    RenderCase::renders("file:///etc", PathConvention::Posix, "/etc"),
    RenderCase::renders("file:///tmp/", PathConvention::Posix, "/tmp/"),
    RenderCase::renders("file:///C:/Project", PathConvention::Posix, "/C:/Project"),
    RenderCase::renders("file:///C:", PathConvention::Posix, "/C:"),
    RenderCase::renders("file:///tmp/%E2%98%83", PathConvention::Posix, "/tmp/☃"),
    RenderCase::renders("file:///tmp/a%5Cb", PathConvention::Posix, "/tmp/a\\b"),
    RenderCase::renders(
        "file:///tmp/100%25/file",
        PathConvention::Posix,
        "/tmp/100%/file",
    ),
    RenderCase::renders(
        "file:///tmp/a%3Fb%23c%25d",
        PathConvention::Posix,
        "/tmp/a?b#c%d",
    ),
    RenderCase::renders("file:///tmp/a%252Fb", PathConvention::Posix, "/tmp/a%2Fb"),
    RenderCase::renders(
        "file:///bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        PathConvention::Posix,
        "/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
    ),
    RenderCase::renders(
        "FILE:///workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::renders(
        "file:/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::renders(
        "file://localhost/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    RenderCase::renders(
        "file://LOCALHOST/workspace/src",
        PathConvention::Posix,
        "/workspace/src",
    ),
    // Windows drive paths.
    RenderCase::renders(
        "file:///C:/Users/Alice%20Smith/src/main.rs",
        PathConvention::Windows,
        r"C:\Users\Alice Smith\src\main.rs",
    ),
    RenderCase::renders("file:///C:/", PathConvention::Windows, "C:\\"),
    RenderCase::renders("file:///C:", PathConvention::Windows, "C:\\"),
    RenderCase::renders("file:///C:/Users", PathConvention::Windows, r"C:\Users"),
    RenderCase::renders("file:///C:/Windows", PathConvention::Windows, r"C:\Windows"),
    RenderCase::renders(
        "file:///d:/snowman/%E2%98%83",
        PathConvention::Windows,
        r"d:\snowman\☃",
    ),
    RenderCase::renders("file:///C:/tmp/", PathConvention::Windows, "C:\\tmp\\"),
    RenderCase::renders(
        "file:///C:/test%20with%20%25/path",
        PathConvention::Windows,
        r"C:\test with %\path",
    ),
    RenderCase::renders(
        "file:///C:/test%20with%20%2525/c%23code",
        PathConvention::Windows,
        r"C:\test with %25\c#code",
    ),
    RenderCase::renders(
        "file:///C:/Source/Z%C3%BCrich%20or%20Zurich%20(%CB%88zj%CA%8A%C9%99r%C9%AAk,/Code/resources/app/plugins/c%23/plugin.json",
        PathConvention::Windows,
        r"C:\Source\Zürich or Zurich (ˈzjʊərɪk,\Code\resources\app\plugins\c#\plugin.json",
    ),
    RenderCase::renders(
        "file:///C:/Users/Abd-al-Haseeb%27s_Dell/Studio/w3mage/wp-content/database.ht.sqlite",
        PathConvention::Windows,
        r"C:\Users\Abd-al-Haseeb's_Dell\Studio\w3mage\wp-content\database.ht.sqlite",
    ),
    RenderCase::renders(
        "file:///C:/project/%25A0.txt",
        PathConvention::Windows,
        r"C:\project\%A0.txt",
    ),
    RenderCase::renders(
        "file:///C:/project/%252e.txt",
        PathConvention::Windows,
        r"C:\project\%2e.txt",
    ),
    // Windows UNC paths.
    RenderCase::renders(
        "file://server/share/src/main.rs",
        PathConvention::Windows,
        r"\\server\share\src\main.rs",
    ),
    RenderCase::renders(
        "file://server/share",
        PathConvention::Windows,
        r"\\server\share",
    ),
    RenderCase::renders(
        "file://server/share/",
        PathConvention::Windows,
        "\\\\server\\share\\",
    ),
    RenderCase::renders(
        "file://shares/files/c%23/p.cs",
        PathConvention::Windows,
        r"\\shares\files\c#\p.cs",
    ),
    RenderCase::renders(
        "file://monacotools1/certificates/SSL/",
        PathConvention::Windows,
        "\\\\monacotools1\\certificates\\SSL\\",
    ),
    // Opaque fallbacks rendered according to their source convention.
    RenderCase::renders(
        "file:///%00/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        PathConvention::Posix,
        "/tmp/null-\0-�-byte",
    ),
    RenderCase::renders(
        "file:///%00/bad/path/XABcAC4AXABDAE8ATQAxAFwA",
        PathConvention::Windows,
        r"\\.\COM1\",
    ),
    RenderCase::renders(
        "file:///%00/bad/path/XABcAD8AXABWAG8AbAB1AG0AZQB7ADAAMAAwADAAMAAwADAAMAAtADAAMAAwADAALQAwADAAMAAwAC0AMAAwADAAMAAtADAAMAAwADAAMAAwADAAMAAwADAAMAAwAH0AXABmAGkAbABlAC4AcgBzAA",
        PathConvention::Windows,
        r"\\?\Volume{00000000-0000-0000-0000-000000000000}\file.rs",
    ),
    // Windows rendering preserves path text without filesystem validation.
    RenderCase::renders("file:///C:/a%3Fb", PathConvention::Windows, "C:\\a?b"),
    RenderCase::renders("file:///C:/a%2Ab", PathConvention::Windows, "C:\\a*b"),
    RenderCase::renders(
        "file:///C:/trailing.",
        PathConvention::Windows,
        "C:\\trailing.",
    ),
    RenderCase::renders(
        "file:///C:/trailing%20",
        PathConvention::Windows,
        "C:\\trailing ",
    ),
    RenderCase::renders(
        "file:///C:/control-%01",
        PathConvention::Windows,
        "C:\\control-\u{1}",
    ),
    RenderCase::renders(
        "file:///C:/file.txt:stream",
        PathConvention::Windows,
        "C:\\file.txt:stream",
    ),
    RenderCase::renders(
        "file://server/sh%3Fare/file.rs",
        PathConvention::Windows,
        "\\\\server\\sh?are\\file.rs",
    ),
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
    // URI segment encodings that cannot be rendered without changing meaning.
    RenderCase::rejects(
        "file:///tmp/non-utf8-%FF",
        PathConvention::Posix,
        ExpectedError::NonUtf8,
    ),
    RenderCase::rejects(
        "file:///tmp/non-utf8-%A0",
        PathConvention::Posix,
        ExpectedError::NonUtf8,
    ),
    RenderCase::rejects(
        "file:///tmp/a%2Fb",
        PathConvention::Posix,
        ExpectedError::EncodedSeparator,
    ),
    RenderCase::rejects(
        "file:///C:/a%2Fb",
        PathConvention::Windows,
        ExpectedError::EncodedSeparator,
    ),
    RenderCase::rejects(
        "file:///C:/a%5Cb",
        PathConvention::Windows,
        ExpectedError::EncodedSeparator,
    ),
];

#[test]
fn renders_native_paths_from_shared_cases() {
    for case in RENDER_CASES {
        let path = PathUri::parse(case.uri).expect("valid file URI");
        let expected = match case.expected {
            RenderExpectation::Rendered(rendered) => Ok(NativePathString(rendered.to_string())),
            RenderExpectation::Error(ExpectedError::OpaqueFallback) => {
                Err(NativePathStringError::OpaqueFallback {
                    path: path.to_string(),
                })
            }
            RenderExpectation::Error(ExpectedError::IncompatibleConvention) => {
                Err(NativePathStringError::IncompatibleConvention {
                    path: path.to_string(),
                    convention: case.convention,
                })
            }
            RenderExpectation::Error(ExpectedError::NonUtf8) => {
                Err(NativePathStringError::NonUtf8 {
                    path: path.to_string(),
                })
            }
            RenderExpectation::Error(ExpectedError::EncodedSeparator) => {
                Err(NativePathStringError::EncodedSeparator {
                    path: path.to_string(),
                    convention: case.convention,
                })
            }
        };

        assert_eq!(
            NativePathString::from_path_uri(&path, case.convention),
            expected,
            "rendering {case:?}"
        );
    }
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
    let path = PathUri::from_path(native_path).expect("absolute native path");

    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Windows),
        Ok(NativePathString(r"C:\bad\�".to_string()))
    );
    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Posix),
        Err(NativePathStringError::OpaqueFallback {
            path: path.to_string(),
        })
    );
}

#[test]
fn serializes_as_a_string() {
    let path = PathUri::parse("file:///workspace/src/lib.rs").expect("valid file URI");
    let rendered = NativePathString::from_path_uri(&path, PathConvention::Posix)
        .expect("POSIX URI should render");

    assert_eq!(
        serde_json::to_string(&rendered).expect("rendered path should serialize"),
        r#""/workspace/src/lib.rs""#
    );
}
