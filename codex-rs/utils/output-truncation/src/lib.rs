//! Helpers for truncating tool and exec output using [`TruncationPolicy`](codex_protocol::protocol::TruncationPolicy).

use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::InputAudio;
pub use codex_utils_string::approx_bytes_for_tokens;
pub use codex_utils_string::approx_token_count;
pub use codex_utils_string::approx_tokens_from_byte_count;
use codex_utils_string::truncate_middle_chars;
use codex_utils_string::truncate_middle_with_token_budget;

pub use codex_protocol::protocol::TruncationPolicy;

const INPUT_AUDIO_JSON_OVERHEAD_BYTES: usize =
    r#"{"type":"input_audio","input_audio":{"data":"","format":""}}"#.len();

pub fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }

    let total_lines = content.lines().count();
    let result = truncate_text(content, policy);
    format!("Total output lines: {total_lines}\n\n{result}")
}

pub fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    match policy {
        TruncationPolicy::Bytes(bytes) => truncate_middle_chars(content, bytes),
        TruncationPolicy::Tokens(tokens) => truncate_middle_with_token_budget(content, tokens).0,
    }
}

pub fn formatted_truncate_text_content_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
) -> (Vec<FunctionCallOutputContentItem>, Option<usize>) {
    let text_segments = items
        .iter()
        .filter_map(|item| match item {
            FunctionCallOutputContentItem::InputText { text } => Some(text.as_str()),
            FunctionCallOutputContentItem::InputImage { .. }
            | FunctionCallOutputContentItem::InputAudio { .. } => None,
        })
        .collect::<Vec<_>>();

    if text_segments.is_empty() {
        return (
            truncate_function_output_items_with_policy(items, policy),
            None,
        );
    }

    let mut combined = String::new();
    for text in &text_segments {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(text);
    }

    let combined_cost = serialized_byte_cost_for_policy(combined.len(), policy);
    let budget = budget_for_policy(policy);
    if combined_cost <= budget {
        let mut remaining_budget = budget.saturating_sub(combined_cost);
        let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
        let mut omitted_audio_items = 0usize;

        for item in items {
            match item {
                FunctionCallOutputContentItem::InputText { text } => {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                }
                FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                    out.push(FunctionCallOutputContentItem::InputImage {
                        image_url: image_url.clone(),
                        detail: *detail,
                    });
                }
                FunctionCallOutputContentItem::InputAudio { input_audio } => {
                    push_audio_item_with_budget(
                        &mut out,
                        input_audio,
                        policy,
                        &mut remaining_budget,
                        &mut omitted_audio_items,
                    );
                }
            }
        }

        push_omitted_audio_summary(&mut out, omitted_audio_items);
        return (out, None);
    }

    let mut out = vec![FunctionCallOutputContentItem::InputText {
        text: formatted_truncate_text(&combined, policy),
    }];
    let mut omitted_audio_items = 0usize;
    for item in items {
        match item {
            FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                    detail: *detail,
                });
            }
            FunctionCallOutputContentItem::InputAudio { .. } => {
                omitted_audio_items += 1;
            }
            FunctionCallOutputContentItem::InputText { .. } => {}
        }
    }
    push_omitted_audio_summary(&mut out, omitted_audio_items);

    (out, Some(approx_token_count(&combined)))
}

pub fn truncate_function_output_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_budget = budget_for_policy(policy);
    let mut omitted_text_items = 0usize;
    let mut omitted_audio_items = 0usize;

    for item in items {
        match item {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining_budget == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let cost = serialized_byte_cost_for_policy(text.len(), policy);

                if cost <= remaining_budget {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining_budget = remaining_budget.saturating_sub(cost);
                } else {
                    let snippet_policy = match policy {
                        TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(remaining_budget),
                        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
                    };
                    let snippet = truncate_text(text, snippet_policy);
                    if snippet.is_empty() {
                        omitted_text_items += 1;
                    } else {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    }
                    remaining_budget = 0;
                }
            }
            FunctionCallOutputContentItem::InputImage { image_url, detail } => {
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                    detail: *detail,
                });
            }
            FunctionCallOutputContentItem::InputAudio { input_audio } => {
                push_audio_item_with_budget(
                    &mut out,
                    input_audio,
                    policy,
                    &mut remaining_budget,
                    &mut omitted_audio_items,
                );
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }
    push_omitted_audio_summary(&mut out, omitted_audio_items);

    out
}

fn budget_for_policy(policy: TruncationPolicy) -> usize {
    match policy {
        TruncationPolicy::Bytes(_) => policy.byte_budget(),
        TruncationPolicy::Tokens(_) => policy.token_budget(),
    }
}

fn serialized_byte_cost_for_policy(byte_count: usize, policy: TruncationPolicy) -> usize {
    match policy {
        TruncationPolicy::Bytes(_) => byte_count,
        TruncationPolicy::Tokens(_) => {
            usize::try_from(approx_tokens_from_byte_count(byte_count)).unwrap_or(usize::MAX)
        }
    }
}

fn push_audio_item_with_budget(
    out: &mut Vec<FunctionCallOutputContentItem>,
    input_audio: &InputAudio,
    policy: TruncationPolicy,
    remaining_budget: &mut usize,
    omitted_audio_items: &mut usize,
) {
    // Preserve audio only when the payload fits the remaining output budget.
    let byte_count = INPUT_AUDIO_JSON_OVERHEAD_BYTES
        .saturating_add(input_audio.data.len())
        .saturating_add(input_audio.format.len());
    let cost = serialized_byte_cost_for_policy(byte_count, policy);
    if cost <= *remaining_budget {
        out.push(FunctionCallOutputContentItem::InputAudio {
            input_audio: input_audio.clone(),
        });
        *remaining_budget = remaining_budget.saturating_sub(cost);
    } else {
        *omitted_audio_items += 1;
    }
}

fn push_omitted_audio_summary(
    out: &mut Vec<FunctionCallOutputContentItem>,
    omitted_audio_items: usize,
) {
    if omitted_audio_items > 0 {
        let item_word = if omitted_audio_items == 1 {
            "item"
        } else {
            "items"
        };
        let owner = if omitted_audio_items == 1 {
            "its"
        } else {
            "their"
        };
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!(
                "[omitted {omitted_audio_items} audio {item_word} because {owner} size exceeds the output truncation budget]"
            ),
        });
    }
}

pub fn approx_tokens_from_byte_count_i64(bytes: i64) -> i64 {
    if bytes <= 0 {
        return 0;
    }

    let bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
    i64::try_from(approx_tokens_from_byte_count(bytes)).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod truncate_tests;
