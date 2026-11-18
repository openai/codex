//! Show the attempted path even when its tab label is truncated.
use crate::style;
use crate::style::StatusTone;
use codex_app_server_client::TypedRequestError;
use codex_utils_path_uri::PathUri;
use color_eyre::Report;
use ratatui::style::Styled;

use ratatui::style::Stylize;
use ratatui::text::Line;

pub(super) fn lines(path: &PathUri, error: &Report, width: u16) -> Vec<Line<'static>> {
    // The server sends the OS message, but uses the same RPC code for unrelated failures.
    // Only classify errors from the file read itself (not, for example, a missing thread).
    let failure = style::status_style(StatusTone::Failure);
    let (heading, hint) = match error.downcast_ref::<TypedRequestError>() {
        Some(TypedRequestError::Server { method, source }) if method == "fs/readFile" => {
            let message = source.message.as_str();
            match message
                .split_once(" (os error ")
                .map_or(message, |(reason, _)| reason)
            {
                "No such file or directory"
                | "The system cannot find the file specified."
                | "The system cannot find the path specified." => (
                    "File not found"
                        .set_style(style::warning_notice_style())
                        .bold(),
                    "This file may have moved since the link was added.".to_owned(),
                ),
                "Permission denied" | "Operation not permitted" | "Access is denied." => (
                    "Permission denied".set_style(failure),
                    "Check your read access, then reopen the link.".to_owned(),
                ),
                _ => ("Could not open file".set_style(failure), message.to_owned()),
            }
        }
        _ => (
            "Could not open file".set_style(failure),
            format!("{error:#}"),
        ),
    };

    let width = usize::from(width);
    let mut lines = vec![Line::from(heading), Line::default()];
    let full_path = path.inferred_native_path_string();
    let basename = path.basename();
    // Split the displayed path, so the directory keeps the owning host's separators.
    let (directory, filename) = basename
        .as_deref()
        .and_then(|name| full_path.strip_suffix(name).map(|prefix| (prefix, name)))
        .unwrap_or(("", &full_path));
    let options =
        textwrap::Options::new(width).word_splitter(textwrap::WordSplitter::NoHyphenation);
    lines.extend(
        textwrap::wrap(filename, &options)
            .into_iter()
            .map(|part| Line::from(part.into_owned()).bold()),
    );
    if !directory.is_empty() {
        let directory_options = options.word_separator(textwrap::WordSeparator::Custom(|s| {
            Box::new(
                s.split_inclusive(['/', '\\'])
                    .map(textwrap::core::Word::from),
            )
        }));
        for part in textwrap::wrap(&format!("in {directory}"), directory_options) {
            lines.push(Line::from(part.into_owned()).set_style(style::secondary_text_style()));
        }
    }
    lines.push(Line::default());
    lines.extend(
        textwrap::wrap(&hint, width)
            .into_iter()
            .map(|line| Line::from(line.into_owned())),
    );
    lines
}
