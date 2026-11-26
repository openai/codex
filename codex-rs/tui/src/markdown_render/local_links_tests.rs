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
        sidebar_file_link("./logs/14:54:42-debug#L12C3"),
        Some(FileLink {
            path: "./logs/14:54:42-debug".into(),
            location: Some(SourceLocation::single(/*line*/ 12, Some(3))),
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

#[test]
fn file_ranges_parse_both_spellings_and_keep_columns() {
    for path in [
        "./src/search.ts",
        "C:/repo/search.ts",
        "./src/C#/f.cs",
        "./logs/14:54:42-debug.txt",
    ] {
        for (suffix, cols) in [
            (":42-44", (None, None)),
            (":42–44", (None, None)),
            ("#L42-L44", (None, None)),
            (":42:5-44:10", (Some(5), Some(10))),
            ("#L42C5-L44C10", (Some(5), Some(10))),
        ] {
            let link = sidebar_file_link(&format!("{path}{suffix}")).unwrap();
            let location = link.location.unwrap();
            let end = location.end.unwrap();
            assert_eq!(link.path, path);
            assert_eq!((location.start.line, end.line), (42, 44));
            assert_eq!((location.start.column, end.column), cols);
        }
    }
    assert_eq!(
        sidebar_file_link("file:///repo/search.ts#L42-L44"),
        sidebar_file_link("/repo/search.ts:42-44")
    );
    assert!(sidebar_file_link("a.ts:42-42").is_some());
}

#[test]
fn file_ranges_reject_malformed_reversed_and_overflowing_coordinates() {
    for suffix in [
        ":42-",
        ":42-44-48",
        ":42-bad",
        ":42-44:bad",
        "#L42-L",
        "#L42-L44:2",
        ":44-42",
        "#L44-L42",
        ":0-2",
        ":42:8-42:3",
        ":42-9999999999999999999999999999999999999",
        "#L42-L0",
    ] {
        assert_eq!(
            sidebar_file_link(&format!("./src/search.ts{suffix}")),
            None,
            "{suffix}"
        );
    }
    assert_eq!(
        sidebar_file_link("./literal%3A42-").unwrap().path,
        "./literal:42-"
    );
}

#[test]
fn file_ranges_inventory_skips_code_and_link_labels() {
    let input = r"src/search.ts:42-44 (src/other.ts#L5-L7). [src/label.ts:1-2](./src/dest.ts#L8-L9)

`src/inline.ts:1-2`

```
src/code.ts:1-2
```

src/_layout.tsx:1-2 src/[id]/route.ts#L1-L2 https://example.org/[id]/route.ts#L2-L3

src/__tests__/util.ts:42-44 https://example.org/__tests__/util.ts:42-44 **src/range.ts:1-2**

See (src/app/(auth)/page.tsx:2-3). See (((group)/page.tsx:2-3)). [log](./logs/14:54:42-debug#L3)

(group)/page.tsx:2-3 [id]/route.ts#L2-L3 \_\_init\_\_.py:1-2 !https://example.org/src/search.ts#L42-L44";
    let expected = [
        "src/search.ts:42-44",
        "src/other.ts#L5-L7",
        "./src/dest.ts#L8-L9",
        "src/_layout.tsx:1-2",
        "src/[id]/route.ts#L1-L2",
        "src/range.ts:1-2",
        "src/app/(auth)/page.tsx:2-3",
        "(group)/page.tsx:2-3",
        "./logs/14:54:42-debug#L3",
        "(group)/page.tsx:2-3",
        "[id]/route.ts#L2-L3",
        "__init__.py:1-2",
    ]
    .map(|dest| sidebar_file_link(dest).unwrap());
    assert_eq!(
        crate::markdown_render::file_links(input, /*cwd*/ None),
        expected
    );
}
