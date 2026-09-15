//! Report loading and grouping metadata for the account dashboard.
//! Replacing a pending load aborts it; its response cannot populate a newer range or account.

use super::models::AccountAnalyticsGrouping as Grouping;
use crate::tui::FrameRequester;
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub(super) const GROUPINGS: [Grouping; 7] = [
    Grouping::Surface,
    Grouping::Feature,
    Grouping::Model,
    Grouping::TaskStart,
    Grouping::Speed,
    Grouping::Reasoning,
    Grouping::TokenType,
];

pub(super) const GROUP_LABELS: [&str; 7] = [
    "Surface",
    "Feature",
    "Model",
    "Turn start",
    "Speed",
    "Reasoning",
    "Token type",
];

pub(super) struct Pending<T> {
    receiver: oneshot::Receiver<Result<Option<T>, String>>,
    task: JoinHandle<()>,
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) enum Load<T> {
    Loading(Pending<T>),
    Ready(T),
    Unavailable,
    Error(String),
}

impl<T: Send + 'static> Load<T> {
    pub(super) fn start(
        future: impl std::future::Future<Output = Result<Option<T>, String>> + Send + 'static,
        frame: FrameRequester,
    ) -> Self {
        let (sender, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            let result = match tokio::time::timeout(
                std::time::Duration::from_secs(/*secs*/ 30),
                AssertUnwindSafe(future).catch_unwind(),
            )
            .await
            {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err("Request interrupted. Press R to retry.".into()),
                Err(_) => Err("Request timed out. Press R to retry.".into()),
            };
            let _ = sender.send(result);
            frame.schedule_frame();
        });
        Self::Loading(Pending { receiver, task })
    }

    pub(super) fn poll(&mut self) {
        if let Self::Loading(pending) = self {
            *self = match pending.receiver.try_recv() {
                Ok(Ok(Some(data))) => Self::Ready(data),
                Ok(Ok(None)) => Self::Unavailable,
                Ok(Err(message)) => Self::Error(message),
                Err(oneshot::error::TryRecvError::Closed) => {
                    Self::Error("Request interrupted. Press R to retry.".into())
                }
                Err(oneshot::error::TryRecvError::Empty) => return,
            };
        }
    }

    pub(super) fn ready(&self) -> Option<&T> {
        if let Self::Ready(value) = self {
            Some(value)
        } else {
            None
        }
    }

    pub(super) fn message(&self) -> Option<&str> {
        match self {
            Self::Ready(_) => None,
            Self::Loading(_) => Some("Loading…"),
            Self::Unavailable => Some("No history has been reported."),
            Self::Error(message) => Some(message),
        }
    }
}

/// Keep tiny refunds visible while avoiding noise on ordinary credit amounts.
pub(super) fn amount(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() < 0.000001 {
        return format!("{value:.2e}");
    }
    let rounded = if value.abs() < 0.01 {
        format!("{value:.6}")
    } else {
        format!("{value:.2}")
    };
    let (whole, fraction) = rounded.split_once('.').unwrap_or((&rounded, ""));
    let mut grouped = String::new();
    for (index, ch) in whole.chars().enumerate() {
        if index > 0
            && ch.is_ascii_digit()
            && (whole.len() - index).is_multiple_of(/*rhs*/ 3)
            && !grouped.ends_with('-')
        {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let fraction = fraction.trim_end_matches('0');
    if !fraction.is_empty() {
        grouped.push('.');
        grouped.push_str(fraction);
    }
    grouped
}

/// Align ordinary credit amounts to two decimals without hiding tiny adjustments.
pub(super) fn credit_amount(value: f64) -> String {
    let formatted = amount(value);
    if value != 0.0 && value.abs() < 0.01 {
        return formatted;
    }
    match formatted.split_once('.') {
        None => format!("{formatted}.00"),
        Some((_, fraction)) if fraction.len() == 1 => format!("{formatted}0"),
        Some(_) => formatted,
    }
}

pub(super) fn date(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|date| date.format("%b %-d").to_string())
        .unwrap_or_else(|_| date.to_string())
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
