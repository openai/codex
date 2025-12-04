use std::{fs, path::Path};

use anstyle::AnsiColor;
use anyhow::{anyhow, Result};
use tree_sitter::Point;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};
use tree_sitter_loader::{Config, Loader};

use super::{
    query_testing::{parse_position_comments, to_utf8_point, Assertion, Utf8Point},
    test::paint,
    util,
};

#[derive(Debug)]
pub struct Failure {
    row: usize,
    column: usize,
    expected_highlight: String,
    actual_highlights: Vec<String>,
}

impl std::error::Error for Failure {}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Failure - row: {}, column: {}, expected highlight '{}', actual highlights: ",
            self.row, self.column, self.expected_highlight
        )?;
        if self.actual_highlights.is_empty() {
            write!(f, "none.")?;
        } else {
            for (i, actual_highlight) in self.actual_highlights.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "'{actual_highlight}'")?;
            }
        }
        Ok(())
    }
}

pub fn test_highlights(
    loader: &Loader,
    loader_config: &Config,
    highlighter: &mut Highlighter,
    directory: &Path,
    use_color: bool,
) -> Result<()> {
    println!("syntax highlighting:");
    test_highlights_indented(loader, loader_config, highlighter, directory, use_color, 2)
}

fn test_highlights_indented(
    loader: &Loader,
    loader_config: &Config,
    highlighter: &mut Highlighter,
    directory: &Path,
    use_color: bool,
    indent_level: usize,
) -> Result<()> {
    let mut failed = false;

    for highlight_test_file in fs::read_dir(directory)? {
        let highlight_test_file = highlight_test_file?;
        let test_file_path = highlight_test_file.path();
        let test_file_name = highlight_test_file.file_name();
        print!(
            "{indent:indent_level$}",
            indent = "",
            indent_level = indent_level * 2
        );
        if test_file_path.is_dir() && test_file_path.read_dir()?.next().is_some() {
            println!("{}:", test_file_name.to_string_lossy());
            if test_highlights_indented(
                loader,
                loader_config,
                highlighter,
                &test_file_path,
                use_color,
                indent_level + 1,
            )
            .is_err()
            {
                failed = true;
            }
        } else {
            let (language, language_config) = loader
                .language_configuration_for_file_name(&test_file_path)?
                .ok_or_else(|| {
                    anyhow!(
                        "{}",
                        util::lang_not_found_for_path(test_file_path.as_path(), loader_config)
                    )
                })?;
            let highlight_config = language_config
                .highlight_config(language, None)?
                .ok_or_else(|| anyhow!("No highlighting config found for {test_file_path:?}"))?;
            match test_highlight(
                loader,
                highlighter,
                highlight_config,
                fs::read(&test_file_path)?.as_slice(),
            ) {
                Ok(assertion_count) => {
                    println!(
                        "✓ {} ({assertion_count} assertions)",
                        paint(
                            use_color.then_some(AnsiColor::Green),
                            test_file_name.to_string_lossy().as_ref()
                        ),
                    );
                }
                Err(e) => {
                    println!(
                        "✗ {}",
                        paint(
                            use_color.then_some(AnsiColor::Red),
                            test_file_name.to_string_lossy().as_ref()
                        )
                    );
                    println!(
                        "{indent:indent_level$}  {e}",
                        indent = "",
                        indent_level = indent_level * 2
                    );
                    failed = true;
                }
            }
        }
    }

    if failed {
        Err(anyhow!(""))
    } else {
        Ok(())
    }
}
pub fn iterate_assertions(
    assertions: &[Assertion],
    highlights: &[(Utf8Point, Utf8Point, Highlight)],
    highlight_names: &[String],
) -> Result<usize> {
    // Iterate through all of the highlighting assertions, checking each one against the
    // actual highlights.
    let mut i = 0;
    let mut actual_highlights = Vec::new();
    for Assertion {
        position,
        length,
        negative,
        expected_capture_name: expected_highlight,
    } in assertions
    {
        let mut passed = false;
        let mut end_column = position.column + length - 1;
        actual_highlights.clear();

        // The assertions are ordered by position, so skip past all of the highlights that
        // end at or before this assertion's position.
        'highlight_loop: while let Some(highlight) = highlights.get(i) {
            if highlight.1 <= *position {
                i += 1;
                continue;
            }

            // Iterate through all of the highlights that start at or before this assertion's
            // position, looking for one that matches the assertion.
            let mut j = i;
            while let (false, Some(highlight)) = (passed, highlights.get(j)) {
                end_column = position.column + length - 1;
                if highlight.0.row >= position.row && highlight.0.column > end_column {
                    break 'highlight_loop;
                }

                // If the highlight matches the assertion, or if the highlight doesn't
                // match the assertion but it's negative, this test passes. Otherwise,
                // add this highlight to the list of actual highlights that span the
                // assertion's position, in order to generate an error message in the event
                // of a failure.
                let highlight_name = &highlight_names[(highlight.2).0];
                if (*highlight_name == *expected_highlight) == *negative {
                    actual_highlights.push(highlight_name);
                } else {
                    passed = true;
                    break 'highlight_loop;
                }

                j += 1;
            }
        }

        if !passed {
            return Err(Failure {
                row: position.row,
                column: end_column,
                expected_highlight: expected_highlight.clone(),
                actual_highlights: actual_highlights.into_iter().cloned().collect(),
            }
            .into());
        }
    }

    Ok(assertions.len())
}

pub fn test_highlight(
    loader: &Loader,
    highlighter: &mut Highlighter,
    highlight_config: &HighlightConfiguration,
    source: &[u8],
) -> Result<usize> {
    // Highlight the file, and parse out all of the highlighting assertions.
    let highlight_names = loader.highlight_names();
    let highlights = get_highlight_positions(loader, highlighter, highlight_config, source)?;
    let assertions =
        parse_position_comments(highlighter.parser(), &highlight_config.language, source)?;

    iterate_assertions(&assertions, &highlights, &highlight_names)
}

pub fn get_highlight_positions(
    loader: &Loader,
    highlighter: &mut Highlighter,
    highlight_config: &HighlightConfiguration,
    source: &[u8],
) -> Result<Vec<(Utf8Point, Utf8Point, Highlight)>> {
    let mut row = 0;
    let mut column = 0;
    let mut byte_offset = 0;
    let mut was_newline = false;
    let mut result = Vec::new();
    let mut highlight_stack = Vec::new();
    let source = String::from_utf8_lossy(source);
    let mut char_indices = source.char_indices();
    for event in highlighter.highlight(highlight_config, source.as_bytes(), None, |string| {
        loader.highlight_config_for_injection_string(string)
    })? {
        match event? {
            HighlightEvent::HighlightStart(h) => highlight_stack.push(h),
            HighlightEvent::HighlightEnd => {
                highlight_stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let mut start_position = Point::new(row, column);
                while byte_offset < end {
                    if byte_offset <= start {
                        start_position = Point::new(row, column);
                    }
                    if let Some((i, c)) = char_indices.next() {
                        if was_newline {
                            row += 1;
                            column = 0;
                        } else {
                            column += i - byte_offset;
                        }
                        was_newline = c == '\n';
                        byte_offset = i;
                    } else {
                        break;
                    }
                }
                if let Some(highlight) = highlight_stack.last() {
                    let utf8_start_position = to_utf8_point(start_position, source.as_bytes());
                    let utf8_end_position =
                        to_utf8_point(Point::new(row, column), source.as_bytes());
                    result.push((utf8_start_position, utf8_end_position, *highlight));
                }
            }
        }
    }
    Ok(result)
}
