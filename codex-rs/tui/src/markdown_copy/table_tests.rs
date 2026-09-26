//! Exercise retained cell coordinates, inline styles, and byte ranges through each layout.

use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn table_layouts_preserve_cell_fragments() {
    for (markdown, expected) in [
        (
            "| **alpha head** | B |\n|---|---|\n| **one two three four** | `value` |\n|  | last |",
            BTreeMap::from([
                ((0, 0), "**alpha head**"),
                ((0, 1), "B"),
                ((1, 0), "**one two three four**"),
                ((1, 1), "`value`"),
                ((2, 0), ""),
                ((2, 1), "last"),
            ]),
        ),
        (
            "| **alpha head** | `a\\|b` |\n|---|---|",
            BTreeMap::from([((0, 0), "**alpha head**"), ((0, 1), "`a|b`")]),
        ),
    ] {
        for width in [4, 12, 80] {
            let lines = crate::markdown_render::render_markdown_lines_with_width_and_cwd(
                markdown,
                Some(width),
                /*cwd*/ None,
            );
            let mut seen = BTreeMap::new();
            let mut identity = None;
            for source in lines.into_iter().filter_map(|line| line.source) {
                let Some(table) = source.copy.as_ref().and_then(|copy| copy.table.as_ref()) else {
                    continue;
                };
                let identity = identity.get_or_insert_with(|| Arc::clone(&table.table));
                assert!(Arc::ptr_eq(identity, &table.table));
                for fragment in &table.fragments {
                    let cell = &fragment.source;
                    assert_eq!(
                        &source.text[fragment.output.clone()],
                        &cell.text[cell.range.clone()],
                        "width {width}, cell ({}, {})",
                        fragment.row,
                        fragment.column,
                    );
                    let markdown = cell.copy.as_ref().unwrap().render(
                        &cell.text,
                        0..cell.text.len(),
                        /*depth*/ 0,
                    );
                    let key = (fragment.row, fragment.column);
                    assert_eq!(markdown, expected[&key]);
                    seen.insert(key, markdown);
                }
            }
            assert_eq!(
                seen,
                expected
                    .iter()
                    .map(|(key, value)| (*key, value.to_string()))
                    .collect()
            );
        }
    }
}
