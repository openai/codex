use super::*;
use crate::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn renders_posix_paths_on_every_host() {
    for (uri, expected) in [
        ("file:///", "/"),
        ("file:///home/alice/src/main.rs", "/home/alice/src/main.rs"),
        ("file:///home/alice/a%20file.rs", "/home/alice/a file.rs"),
        ("file:///workspace/src/lib.rs", "/workspace/src/lib.rs"),
        (
            "file:///workspace/tests/test.rs",
            "/workspace/tests/test.rs",
        ),
        ("file:///etc", "/etc"),
        ("file:///tmp/", "/tmp/"),
        ("file:///C:/Project", "/C:/Project"),
        ("file:///C:", "/C:"),
        ("file:///tmp/%E2%98%83", "/tmp/☃"),
        ("file:///tmp/a%5Cb", "/tmp/a\\b"),
        ("file:///tmp/100%25/file", "/tmp/100%/file"),
        ("file:///tmp/a%3Fb%23c%25d", "/tmp/a?b#c%d"),
        ("file:///tmp/a%252Fb", "/tmp/a%2Fb"),
        (
            "file:///bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
            "/bad/path/L3RtcC9udWxsLQAt_y1ieXRl",
        ),
        ("FILE:///workspace/src", "/workspace/src"),
        ("file:/workspace/src", "/workspace/src"),
        ("file://localhost/workspace/src", "/workspace/src"),
        ("file://LOCALHOST/workspace/src", "/workspace/src"),
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert_eq!(
            NativePathString::from_path_uri(&path, PathConvention::Posix)
                .map(NativePathString::into_string),
            Ok(expected.to_string()),
            "rendering {uri}"
        );
    }
}

#[test]
fn renders_windows_drive_paths_on_every_host() {
    for (uri, expected) in [
        (
            "file:///C:/Users/Alice%20Smith/src/main.rs",
            r"C:\Users\Alice Smith\src\main.rs",
        ),
        ("file:///C:/", "C:\\"),
        ("file:///C:", "C:\\"),
        ("file:///C:/Users", r"C:\Users"),
        ("file:///C:/Windows", r"C:\Windows"),
        ("file:///d:/snowman/%E2%98%83", r"d:\snowman\☃"),
        ("file:///C:/tmp/", "C:\\tmp\\"),
        ("file:///C:/test%20with%20%25/path", r"C:\test with %\path"),
        (
            "file:///C:/test%20with%20%2525/c%23code",
            r"C:\test with %25\c#code",
        ),
        (
            "file:///C:/Source/Z%C3%BCrich%20or%20Zurich%20(%CB%88zj%CA%8A%C9%99r%C9%AAk,/Code/resources/app/plugins/c%23/plugin.json",
            r"C:\Source\Zürich or Zurich (ˈzjʊərɪk,\Code\resources\app\plugins\c#\plugin.json",
        ),
        (
            "file:///C:/Users/Abd-al-Haseeb%27s_Dell/Studio/w3mage/wp-content/database.ht.sqlite",
            r"C:\Users\Abd-al-Haseeb's_Dell\Studio\w3mage\wp-content\database.ht.sqlite",
        ),
        ("file:///C:/project/%25A0.txt", r"C:\project\%A0.txt"),
        ("file:///C:/project/%252e.txt", r"C:\project\%2e.txt"),
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert_eq!(
            NativePathString::from_path_uri(&path, PathConvention::Windows)
                .map(NativePathString::into_string),
            Ok(expected.to_string()),
            "rendering {uri}"
        );
    }
}

#[test]
fn renders_windows_unc_paths_on_every_host() {
    for (uri, expected) in [
        (
            "file://server/share/src/main.rs",
            r"\\server\share\src\main.rs",
        ),
        ("file://server/share", r"\\server\share"),
        ("file://server/share/", "\\\\server\\share\\"),
        ("file://shares/files/c%23/p.cs", r"\\shares\files\c#\p.cs"),
        (
            "file://monacotools1/certificates/SSL/",
            "\\\\monacotools1\\certificates\\SSL\\",
        ),
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert_eq!(
            NativePathString::from_path_uri(&path, PathConvention::Windows)
                .map(NativePathString::into_string),
            Ok(expected.to_string()),
            "rendering {uri}"
        );
    }
}

#[test]
fn rejects_paths_incompatible_with_the_convention() {
    for (uri, convention) in [
        ("file://server/share/file.txt", PathConvention::Posix),
        ("file://server/share/file.rs", PathConvention::Posix),
        ("file:///usr/local/file.txt", PathConvention::Windows),
        ("file:///home/alice/file.rs", PathConvention::Windows),
        ("file://server/", PathConvention::Windows),
        ("file:///_:/path", PathConvention::Windows),
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert!(matches!(
            NativePathString::from_path_uri(&path, convention),
            Err(NativePathStringError::IncompatibleConvention { .. })
        ));
    }
}

#[test]
fn rejects_opaque_fallback_paths_that_cannot_be_recovered() {
    let path = PathUri::parse("file:///%00/bad/path/YQ").expect("canonical opaque fallback URI");

    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::native()),
        Err(NativePathStringError::OpaqueFallback {
            path: path.to_string(),
        })
    );
}

#[cfg(unix)]
#[test]
fn renders_native_opaque_fallback_paths_lossily() {
    use std::os::unix::ffi::OsStringExt;

    let native_path = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/null-\0-\xff-byte".to_vec(),
    ));
    let path = PathUri::from_path(native_path).expect("absolute native path");

    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Posix)
            .map(NativePathString::into_string),
        Ok("/tmp/null-\0-�-byte".to_string())
    );
    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Windows),
        Err(NativePathStringError::IncompatibleConvention {
            path: path.to_string(),
            convention: PathConvention::Windows,
        })
    );
}

#[cfg(windows)]
#[test]
fn renders_windows_namespace_fallback_paths() {
    for native_path in [
        r"\\.\COM1",
        r"\\?\Volume{00000000-0000-0000-0000-000000000000}\file.rs",
    ] {
        let path = PathUri::from_path(native_path).expect("absolute Windows namespace path");

        assert_eq!(
            NativePathString::from_path_uri(&path, PathConvention::Windows)
                .map(NativePathString::into_string),
            Ok(native_path.to_string()),
            "rendering {native_path}"
        );
    }
}

#[cfg(windows)]
#[test]
fn renders_native_opaque_fallback_paths_lossily() {
    use std::os::windows::ffi::OsStringExt;

    let native_path = std::path::PathBuf::from(std::ffi::OsString::from_wide(
        &r"C:\bad\"
            .encode_utf16()
            .chain([0xd800])
            .collect::<Vec<_>>(),
    ));
    let path = PathUri::from_path(native_path).expect("absolute native path");

    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Windows)
            .map(NativePathString::into_string),
        Ok(r"C:\bad\�".to_string())
    );
    assert_eq!(
        NativePathString::from_path_uri(&path, PathConvention::Posix),
        Err(NativePathStringError::IncompatibleConvention {
            path: path.to_string(),
            convention: PathConvention::Posix,
        })
    );
}

#[test]
fn rejects_non_utf8_paths() {
    for uri in ["file:///tmp/non-utf8-%FF", "file:///tmp/non-utf8-%A0"] {
        let path = PathUri::parse(uri).expect("valid file URI");

        assert!(matches!(
            NativePathString::from_path_uri(&path, PathConvention::Posix),
            Err(NativePathStringError::NonUtf8 { .. })
        ));
    }
}

#[test]
fn rejects_encoded_separators() {
    for (uri, convention) in [
        ("file:///tmp/a%2Fb", PathConvention::Posix),
        ("file:///C:/a%2Fb", PathConvention::Windows),
        ("file:///C:/a%5Cb", PathConvention::Windows),
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert!(matches!(
            NativePathString::from_path_uri(&path, convention),
            Err(NativePathStringError::EncodedSeparator { .. })
        ));
    }
}

#[test]
fn rejects_invalid_windows_components() {
    for uri in [
        "file:///C:/a%3Fb",
        "file:///C:/a%2Ab",
        "file:///C:/trailing.",
        "file:///C:/trailing%20",
        "file:///C:/control-%01",
        "file://server/sh%3Fare/file.rs",
    ] {
        let path = PathUri::parse(uri).expect("valid file URI");
        assert!(matches!(
            NativePathString::from_path_uri(&path, PathConvention::Windows),
            Err(NativePathStringError::InvalidWindowsComponent { .. })
        ));
    }
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
