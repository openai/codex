use std::{fs, path::Path, sync::LazyLock};

use anyhow::{anyhow, Result};
use bstr::{BStr, ByteSlice};
use regex::Regex;
use tree_sitter::{Language, Parser, Point};

static CAPTURE_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("[\\w_\\-.]+").unwrap());

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Utf8Point {
    pub row: usize,
    pub column: usize,
}

impl std::fmt::Display for Utf8Point {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.row, self.column)
    }
}

impl Utf8Point {
    #[must_use]
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

#[must_use]
pub fn to_utf8_point(point: Point, source: &[u8]) -> Utf8Point {
    if point.column == 0 {
        return Utf8Point::new(point.row, 0);
    }

    let bstr = BStr::new(source);
    let line = bstr.lines_with_terminator().nth(point.row).unwrap();
    let mut utf8_column = 0;

    for (_, grapheme_end, _) in line.grapheme_indices() {
        utf8_column += 1;
        if grapheme_end >= point.column {
            break;
        }
    }

    Utf8Point {
        row: point.row,
        column: utf8_column,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CaptureInfo {
    pub name: String,
    pub start: Utf8Point,
    pub end: Utf8Point,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Assertion {
    pub position: Utf8Point,
    pub length: usize,
    pub negative: bool,
    pub expected_capture_name: String,
}

impl Assertion {
    #[must_use]
    pub const fn new(
        row: usize,
        col: usize,
        length: usize,
        negative: bool,
        expected_capture_name: String,
    ) -> Self {
        Self {
            position: Utf8Point::new(row, col),
            length,
            negative,
            expected_capture_name,
        }
    }
}

/// Parse the given source code, finding all of the comments that contain
/// highlighting assertions. Return a vector of (position, expected highlight name)
/// pairs.
pub fn parse_position_comments(
    parser: &mut Parser,
    language: &Language,
    source: &[u8],
) -> Result<Vec<Assertion>> {
    let mut result = Vec::new();
    let mut assertion_ranges = Vec::new();

    // Parse the code.
    parser.set_included_ranges(&[]).unwrap();
    parser.set_language(language).unwrap();
    let tree = parser.parse(source, None).unwrap();

    // Walk the tree, finding comment nodes that contain assertions.
    let mut ascending = false;
    let mut cursor = tree.root_node().walk();
    loop {
        if ascending {
            let node = cursor.node();

            // Find every comment node.
            if node.kind().to_lowercase().contains("comment") {
                if let Ok(text) = node.utf8_text(source) {
                    let mut position = node.start_position();
                    if position.row > 0 {
                        // Find the arrow character ("^" or "<-") in the comment. A left arrow
                        // refers to the column where the comment node starts. An up arrow refers
                        // to its own column.
                        let mut has_left_caret = false;
                        let mut has_arrow = false;
                        let mut negative = false;
                        let mut arrow_end = 0;
                        let mut arrow_count = 1;
                        for (i, c) in text.char_indices() {
                            arrow_end = i + 1;
                            if c == '-' && has_left_caret {
                                has_arrow = true;
                                break;
                            }
                            if c == '^' {
                                has_arrow = true;
                                position.column += i;
                                // Continue counting remaining arrows and update their end column
                                for (_, c) in text[arrow_end..].char_indices() {
                                    if c != '^' {
                                        arrow_end += arrow_count - 1;
                                        break;
                                    }
                                    arrow_count += 1;
                                }
                                break;
                            }
                            has_left_caret = c == '<';
                        }

                        // find any ! after arrows but before capture name
                        if has_arrow {
                            for (i, c) in text[arrow_end..].char_indices() {
                                if c == '!' {
                                    negative = true;
                                    arrow_end += i + 1;
                                    break;
                                } else if !c.is_whitespace() {
                                    break;
                                }
                            }
                        }

                        // If the comment node contains an arrow and a highlight name, record the
                        // highlight name and the position.
                        if let (true, Some(mat)) =
                            (has_arrow, CAPTURE_NAME_REGEX.find(&text[arrow_end..]))
                        {
                            assertion_ranges.push((node.start_position(), node.end_position()));
                            result.push(Assertion {
                                position: to_utf8_point(position, source),
                                length: arrow_count,
                                negative,
                                expected_capture_name: mat.as_str().to_string(),
                            });
                        }
                    }
                }
            }

            // Continue walking the tree.
            if cursor.goto_next_sibling() {
                ascending = false;
            } else if !cursor.goto_parent() {
                break;
            }
        } else if !cursor.goto_first_child() {
            ascending = true;
        }
    }

    // Adjust the row number in each assertion's position to refer to the line of
    // code *above* the assertion. There can be multiple lines of assertion comments and empty
    // lines, so the positions may have to be decremented by more than one row.
    let mut i = 0;
    let lines = source.lines_with_terminator().collect::<Vec<_>>();
    for assertion in &mut result {
        let original_position = assertion.position;
        loop {
            let on_assertion_line = assertion_ranges[i..]
                .iter()
                .any(|(start, _)| start.row == assertion.position.row);
            let on_empty_line = lines[assertion.position.row].len() <= assertion.position.column;
            if on_assertion_line || on_empty_line {
                if assertion.position.row > 0 {
                    assertion.position.row -= 1;
                } else {
                    return Err(anyhow!(
                        "Error: could not find a line that corresponds to the assertion `{}` located at {original_position}",
                        assertion.expected_capture_name
                    ));
                }
            } else {
                while i < assertion_ranges.len()
                    && assertion_ranges[i].0.row < assertion.position.row
                {
                    i += 1;
                }
                break;
            }
        }
    }

    // The assertions can end up out of order due to the line adjustments.
    result.sort_unstable_by_key(|a| a.position);

    Ok(result)
}

pub fn assert_expected_captures(
    infos: &[CaptureInfo],
    path: &Path,
    parser: &mut Parser,
    language: &Language,
) -> Result<usize> {
    let contents = fs::read_to_string(path)?;
    let pairs = parse_position_comments(parser, language, contents.as_bytes())?;
    for assertion in &pairs {
        if let Some(found) = &infos.iter().find(|p| {
            assertion.position >= p.start
                && (assertion.position.row < p.end.row
                    || assertion.position.column + assertion.length - 1 < p.end.column)
        }) {
            if assertion.expected_capture_name != found.name && found.name != "name" {
                return Err(anyhow!(
                    "Assertion failed: at {}, found {}, expected {}",
                    found.start,
                    assertion.expected_capture_name,
                    found.name
                ));
            }
        } else {
            return Err(anyhow!(
                "Assertion failed: could not match {} at row {}, column {}",
                assertion.expected_capture_name,
                assertion.position.row,
                assertion.position.column + assertion.length - 1,
            ));
        }
    }
    Ok(pairs.len())
}
