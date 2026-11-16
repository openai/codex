//! Tests for local-link parsing and root-preserving separator normalization.

use super::FileLink;
use super::SourceLocation;
use super::sidebar_file_link;
use super::trim_trailing_local_path_separator;
use pretty_assertions::assert_eq;

#[test]
fn trailing_separator_trimming_preserves_local_roots() {
    let paths = ["/", "//", "C:/", "dir/", "//server/share/", "C:/dir/"];
    assert_eq!(
        paths.map(trim_trailing_local_path_separator),
        ["/", "//", "C:/", "dir", "//server/share", "C:/dir"]
    );
}

#[test]
fn sidebar_keeps_line_and_column_for_extensionless_source_links() {
    assert_eq!(
        sidebar_file_link("./Makefile#L12C3"),
        Some(FileLink {
            path: "./Makefile".into(),
            location: Some(SourceLocation {
                line: 12,
                column: 3
            }),
        }),
    );
    for path in ["Makefile", "build/Makefile"] {
        assert_eq!(
            sidebar_file_link(path),
            Some(FileLink {
                path: path.into(),
                location: None,
            }),
        );
    }
    assert_eq!(sidebar_file_link("https://example.org/Makefile#L12"), None);
}
