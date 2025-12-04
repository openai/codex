use std::{fs, path::Path};

use anstyle::AnsiColor;
use anyhow::{anyhow, Result};
use tree_sitter_loader::{Config, Loader};
use tree_sitter_tags::{TagsConfiguration, TagsContext};

use super::{
    query_testing::{parse_position_comments, to_utf8_point, Assertion, Utf8Point},
    test::paint,
    util,
};

#[derive(Debug)]
pub struct Failure {
    row: usize,
    column: usize,
    expected_tag: String,
    actual_tags: Vec<String>,
}

impl std::error::Error for Failure {}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Failure - row: {}, column: {}, expected tag: '{}', actual tag: ",
            self.row, self.column, self.expected_tag
        )?;
        if self.actual_tags.is_empty() {
            write!(f, "none.")?;
        } else {
            for (i, actual_tag) in self.actual_tags.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "'{actual_tag}'")?;
            }
        }
        Ok(())
    }
}

pub fn test_tags(
    loader: &Loader,
    loader_config: &Config,
    tags_context: &mut TagsContext,
    directory: &Path,
    use_color: bool,
) -> Result<()> {
    println!("tags:");
    test_tags_indented(loader, loader_config, tags_context, directory, use_color, 2)
}

pub fn test_tags_indented(
    loader: &Loader,
    loader_config: &Config,
    tags_context: &mut TagsContext,
    directory: &Path,
    use_color: bool,
    indent_level: usize,
) -> Result<()> {
    let mut failed = false;

    for tag_test_file in fs::read_dir(directory)? {
        let tag_test_file = tag_test_file?;
        let test_file_path = tag_test_file.path();
        let test_file_name = tag_test_file.file_name();
        print!(
            "{indent:indent_level$}",
            indent = "",
            indent_level = indent_level * 2
        );
        if test_file_path.is_dir() && test_file_path.read_dir()?.next().is_some() {
            println!("{}:", test_file_name.to_string_lossy());
            if test_tags_indented(
                loader,
                loader_config,
                tags_context,
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
            let tags_config = language_config
                .tags_config(language)?
                .ok_or_else(|| anyhow!("No tags config found for {test_file_path:?}"))?;
            match test_tag(
                tags_context,
                tags_config,
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

pub fn test_tag(
    tags_context: &mut TagsContext,
    tags_config: &TagsConfiguration,
    source: &[u8],
) -> Result<usize> {
    let tags = get_tag_positions(tags_context, tags_config, source)?;
    let assertions = parse_position_comments(tags_context.parser(), &tags_config.language, source)?;

    // Iterate through all of the assertions, checking against the actual tags.
    let mut i = 0;
    let mut actual_tags = Vec::<&String>::new();
    for Assertion {
        position,
        length,
        negative,
        expected_capture_name: expected_tag,
    } in &assertions
    {
        let mut passed = false;
        let mut end_column = position.column + length - 1;

        'tag_loop: while let Some(tag) = tags.get(i) {
            if tag.1 <= *position {
                i += 1;
                continue;
            }

            // Iterate through all of the tags that start at or before this assertion's
            // position, looking for one that matches the assertion
            let mut j = i;
            while let (false, Some(tag)) = (passed, tags.get(j)) {
                end_column = position.column + length - 1;
                if tag.0.column > end_column {
                    break 'tag_loop;
                }

                let tag_name = &tag.2;
                if (*tag_name == *expected_tag) == *negative {
                    actual_tags.push(tag_name);
                } else {
                    passed = true;
                    break 'tag_loop;
                }

                j += 1;
                if tag == tags.last().unwrap() {
                    break 'tag_loop;
                }
            }
        }

        if !passed {
            return Err(Failure {
                row: position.row,
                column: end_column,
                expected_tag: expected_tag.clone(),
                actual_tags: actual_tags.into_iter().cloned().collect(),
            }
            .into());
        }
    }

    Ok(assertions.len())
}

pub fn get_tag_positions(
    tags_context: &mut TagsContext,
    tags_config: &TagsConfiguration,
    source: &[u8],
) -> Result<Vec<(Utf8Point, Utf8Point, String)>> {
    let (tags_iter, _has_error) = tags_context.generate_tags(tags_config, source, None)?;
    let tag_positions = tags_iter
        .filter_map(std::result::Result::ok)
        .map(|tag| {
            let tag_postfix = tags_config.syntax_type_name(tag.syntax_type_id).to_string();
            let tag_name = if tag.is_definition {
                format!("definition.{tag_postfix}")
            } else {
                format!("reference.{tag_postfix}")
            };
            (
                to_utf8_point(tag.span.start, source),
                to_utf8_point(tag.span.end, source),
                tag_name,
            )
        })
        .collect();
    Ok(tag_positions)
}
