//! Copy targets and bounded previews in the shared picker.
//!
//! Preview truncation never changes the source text or format sent to the clipboard.

use super::*;
use crate::clipboard_copy::CopyFormat;
use crate::text_formatting::truncate_text;

impl ChatWidget {
    pub(super) fn show_copy_picker(&mut self) {
        let mut choices = Vec::new();
        if let Some(status_targets) = &self.transcript.last_status_copy_targets {
            choices.push((
                "Whole status".to_string(),
                Arc::<str>::from(status_targets.handle.copy_text()),
                CopyFormat::PlainText,
            ));
            choices.extend(
                status_targets
                    .fields
                    .iter()
                    .cloned()
                    .map(|(label, text)| (label, text, CopyFormat::PlainText)),
            );
        } else if let Some(markdown) = self
            .transcript
            .last_agent_markdown
            .as_deref()
            .filter(|markdown| !markdown.is_empty())
        {
            choices.push((
                "Whole response".to_string(),
                Arc::<str>::from(markdown),
                CopyFormat::Markdown,
            ));
            let source = self
                .transcript
                .last_agent_source
                .as_deref()
                .unwrap_or(markdown);
            choices.extend(
                crate::markdown::extract_copy_targets(source)
                    .into_iter()
                    .filter_map(|target| match target {
                        crate::markdown::CopyTarget::Code { language, content } => Some((
                            language.map_or_else(
                                || "Code block".to_string(),
                                |language| format!("{language} code"),
                            ),
                            content,
                            CopyFormat::PlainText,
                        )),
                        crate::markdown::CopyTarget::Quote(content) => {
                            let content: String = content
                                .split_inclusive('\n')
                                .map(|line| {
                                    crate::git_action_directives::strip_line_directives(line).0
                                })
                                .collect();
                            (!content.trim().is_empty()).then(|| {
                                (
                                    "Blockquote".to_string(),
                                    Arc::from(content),
                                    CopyFormat::PlainText,
                                )
                            })
                        }
                    }),
            );
        }
        if choices.is_empty() {
            self.copy_last_agent_markdown();
            return;
        }

        let items = choices
            .into_iter()
            .map(|(label, text, format)| {
                let description = text
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| truncate_text(line.trim(), /*max_graphemes*/ 72));
                SelectionItem {
                    name: label.clone(),
                    description,
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::CopySelection {
                            text: Arc::clone(&text),
                            label: label.clone(),
                            format,
                        });
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        self.show_selection_view(SelectionViewParams {
            title: Some("Copy to clipboard".into()),
            items,
            ..SelectionViewParams::picker()
        });
        self.defer_input_until_settings_applied();
    }
}
