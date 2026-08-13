//! Live-updating, thread-scoped billing details embedded in `/status` history cards.

use super::format::FieldFormatter;
use super::format::push_label;
use super::helpers::format_tokens_compact;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::ThreadUsage;
use codex_app_server_protocol::ThreadUsageBreakdownGroup;
use ratatui::prelude::Line;
use ratatui::style::Stylize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

const REASONING_ORDER: [&str; 8] = [
    "None",
    "Minimal",
    "Light",
    "Medium",
    "High",
    "Extra High",
    "Max",
    "Ultra",
];
const SPEED_ORDER: [&str; 3] = ["Fast mode", "Ultrafast", "Standard"];
const MODELS_LABEL: &str = "  Models";
const REASONING_LABEL: &str = "  Reasoning";
const SPEED_LABEL: &str = "  Speed";
const BILLED_TOKENS_LABEL: &str = "  Billed tokens";

/// Shared state updates a committed `/status` card when its asynchronous estimate arrives.
#[derive(Clone, Debug, Default)]
pub(crate) struct StatusThreadUsage {
    estimate: Arc<RwLock<Option<ThreadUsage>>>,
    reserve_label_width: Arc<AtomicBool>,
}

impl StatusThreadUsage {
    pub(crate) fn reserve_label_width(&self) {
        self.reserve_label_width
            .store(/*val*/ true, Ordering::Relaxed);
    }

    pub(crate) fn set_estimate(&self, estimate: Option<ThreadUsage>) {
        #[expect(clippy::expect_used)]
        let mut stored_estimate = self
            .estimate
            .write()
            .expect("status history thread-usage state poisoned");
        *stored_estimate = estimate;
    }

    pub(crate) fn push_labels(&self, labels: &mut Vec<String>, seen: &mut BTreeSet<String>) {
        // Keep existing card rows stable when asynchronous billing details arrive. Reserving a
        // formatter label does not emit a loading placeholder or an empty billing row.
        if self.reserve_label_width.load(Ordering::Relaxed) {
            push_label(labels, seen, BILLED_TOKENS_LABEL);
        }
        #[expect(clippy::expect_used)]
        let stored_estimate = self
            .estimate
            .read()
            .expect("status history thread-usage state poisoned");
        let Some(estimate) = stored_estimate.as_ref() else {
            return;
        };
        push_label(labels, seen, "Thread usage");
        if !estimate.groups.is_empty() {
            push_label(labels, seen, MODELS_LABEL);
            push_label(labels, seen, REASONING_LABEL);
            push_label(labels, seen, SPEED_LABEL);
            if estimate
                .groups
                .iter()
                .any(|group| group.input_tokens.is_some() || group.output_tokens.is_some())
            {
                push_label(labels, seen, BILLED_TOKENS_LABEL);
            }
        }
    }

    pub(crate) fn lines(
        &self,
        formatter: &FieldFormatter,
        value_width: usize,
    ) -> Vec<Line<'static>> {
        #[expect(clippy::expect_used)]
        let stored_estimate = self
            .estimate
            .read()
            .expect("status history thread-usage state poisoned");
        let Some(estimate) = stored_estimate.as_ref() else {
            return Vec::new();
        };

        let credits_micros = estimate.estimated_usage_credits_micros;
        let mut usage = vec![format!("{} credits", format_credit_micros(credits_micros)).into()];
        if let Some(cost) = estimate
            .estimated_usage_usd_micros
            .and_then(format_estimated_usd_micros)
        {
            usage.push(" · ".dim());
            usage.push(cost.into());
        }
        let mut lines = vec![formatter.line("Thread usage", usage)];

        for (label, dimension) in [
            (MODELS_LABEL, BreakdownDimension::Model),
            (REASONING_LABEL, BreakdownDimension::Reasoning),
            (SPEED_LABEL, BreakdownDimension::Speed),
        ] {
            let Some(value) = grouped_usage(&estimate.groups, dimension) else {
                continue;
            };
            let mut wrapped = word_wrap_lines(
                [Line::from(value)],
                RtOptions::new(value_width.max(/*other*/ 1)),
            )
            .into_iter();
            if let Some(first) = wrapped.next() {
                lines.push(formatter.line(label, first.spans));
                lines.extend(wrapped.map(|line| formatter.continuation(line.spans)));
            }
        }

        let sum_tokens = |count: fn(&ThreadUsageBreakdownGroup) -> Option<i64>| {
            estimate
                .groups
                .iter()
                .try_fold(/*init*/ 0_i64, |total, group| {
                    count(group).map(|tokens| total.saturating_add(tokens))
                })
        };
        let input_tokens = sum_tokens(|group| group.input_tokens);
        let cached_tokens = sum_tokens(|group| group.cached_input_tokens);
        let output_tokens = sum_tokens(|group| group.output_tokens);
        if !estimate.groups.is_empty() && (input_tokens.is_some() || output_tokens.is_some()) {
            let mut tokens = Vec::new();
            if let Some(input_tokens) = input_tokens {
                tokens.push(format!("{} input", format_tokens_compact(input_tokens)));
                if let Some(cached_tokens) = cached_tokens {
                    tokens.push(format!("({} cached)", format_tokens_compact(cached_tokens)));
                }
            }
            if let Some(output_tokens) = output_tokens {
                if !tokens.is_empty() {
                    tokens.push("+".to_string());
                }
                tokens.push(format!("{} output", format_tokens_compact(output_tokens)));
            }
            lines.push(formatter.line(BILLED_TOKENS_LABEL, vec![tokens.join(" ").into()]));
        }

        lines
    }
}

#[derive(Clone, Copy)]
enum BreakdownDimension {
    Model,
    Reasoning,
    Speed,
}

fn grouped_usage(
    groups: &[ThreadUsageBreakdownGroup],
    dimension: BreakdownDimension,
) -> Option<String> {
    let mut totals = BTreeMap::<String, i64>::new();
    for group in groups {
        if group.estimated_usage_credits_micros < 0 {
            continue;
        }
        let value = match dimension {
            BreakdownDimension::Model => group
                .model
                .as_deref()
                .map(format_model_display_name)
                .unwrap_or_else(|| "Other".to_string()),
            BreakdownDimension::Reasoning => match group.reasoning_effort.as_deref() {
                Some("none") => "None",
                Some("minimal") => "Minimal",
                Some("low") => "Light",
                Some("medium") => "Medium",
                Some("high") => "High",
                Some("xhigh") => "Extra High",
                Some("max") => "Max",
                Some("ultra") => "Ultra",
                Some(other) => other,
                None => "Other",
            }
            .to_string(),
            BreakdownDimension::Speed => match group.speed.as_deref() {
                Some("fast") => "Fast mode",
                Some("ultrafast") => "Ultrafast",
                Some("standard") => "Standard",
                Some(other) => other,
                None => "Other",
            }
            .to_string(),
        };
        let total = totals.entry(value).or_default();
        *total = total.saturating_add(group.estimated_usage_credits_micros);
    }
    if totals.is_empty() {
        return None;
    }

    let total_credits_micros =
        totals.values().fold(
            /*init*/ 0_i64,
            |total, credits| total.saturating_add(*credits),
        );
    let mut totals = totals.into_iter().collect::<Vec<_>>();
    totals.sort_by(
        |(left_name, left_credits), (right_name, right_credits)| match dimension {
            BreakdownDimension::Model => right_credits
                .cmp(left_credits)
                .then_with(|| left_name.cmp(right_name)),
            BreakdownDimension::Reasoning => REASONING_ORDER
                .iter()
                .position(|value| value == left_name)
                .unwrap_or(usize::MAX)
                .cmp(
                    &REASONING_ORDER
                        .iter()
                        .position(|value| value == right_name)
                        .unwrap_or(usize::MAX),
                )
                .then_with(|| left_name.cmp(right_name)),
            BreakdownDimension::Speed => SPEED_ORDER
                .iter()
                .position(|value| value == left_name)
                .unwrap_or(usize::MAX)
                .cmp(
                    &SPEED_ORDER
                        .iter()
                        .position(|value| value == right_name)
                        .unwrap_or(usize::MAX),
                )
                .then_with(|| left_name.cmp(right_name)),
        },
    );
    Some(
        totals
            .into_iter()
            .map(|(name, credits)| {
                let percentage = if total_credits_micros <= 0 {
                    0
                } else {
                    (i128::from(credits) * 100 + i128::from(total_credits_micros) / 2)
                        / i128::from(total_credits_micros)
                };
                format!("{name} {percentage}%")
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn format_model_display_name(model: &str) -> String {
    let Some((prefix, remainder)) = model.split_once('-') else {
        return model.to_string();
    };
    if !prefix.eq_ignore_ascii_case("gpt") || remainder.is_empty() {
        return model.to_string();
    }

    let mut segments = remainder.split('-');
    let first = segments.next().unwrap_or_default();
    let mut display_name = format!("GPT-{first}");
    let numeric_model = first
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit());
    for segment in segments {
        display_name.push(if numeric_model { ' ' } else { '-' });
        if segment.eq_ignore_ascii_case("oai") {
            display_name.push_str("OAI");
            continue;
        }

        let mut characters = segment.chars();
        if let Some(first_character) = characters.next() {
            display_name.extend(first_character.to_uppercase());
            display_name.push_str(characters.as_str());
        }
    }

    display_name
}

pub(crate) fn format_credit_micros(micros: i64) -> String {
    let micros = i128::from(micros.max(/*other*/ 0));
    let (unit_micros, suffix) = [
        (1_000_000_000_000_000_000_i128, "T"),
        (1_000_000_000_000_000_i128, "B"),
        (1_000_000_000_000_i128, "M"),
        (1_000_000_000_i128, "K"),
    ]
    .into_iter()
    .find(|(unit, _)| micros >= *unit)
    .unwrap_or((1_000_000, ""));
    let tenths = (micros * 10 + unit_micros / 2) / unit_micros;
    let whole = tenths / 10;
    let fraction = tenths % 10;
    if fraction == 0 {
        format!("{whole}{suffix}")
    } else {
        format!("{whole}.{fraction}{suffix}")
    }
}

pub(crate) fn format_estimated_usd_micros(micros: i64) -> Option<String> {
    if micros < 0 {
        return None;
    }
    if micros > 0 && micros < 100 {
        return Some(format!("~$0.{micros:06}"));
    }
    if micros > 0 && micros < 10_000 {
        let ten_thousandths = micros.saturating_add(/*rhs*/ 50) / 100;
        return Some(format!("~$0.{ten_thousandths:04}"));
    }

    let cents = micros.saturating_add(/*rhs*/ 5_000) / 10_000;
    Some(format!("~${}.{:02}", cents / 100, cents % 100))
}

#[cfg(test)]
#[path = "thread_usage_tests.rs"]
mod tests;
