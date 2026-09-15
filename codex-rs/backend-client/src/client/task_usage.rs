//! Consumer task accounting: current allowance percentages and actual balance debits.
//! This contract is deliberately separate from enterprise estimated lifetime credits.

use super::Client;
use super::PathStyle;
use super::RequestError;
use http::Method;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize)]
pub struct TaskUsageThread {
    pub thread_id: String,
    pub created_at: Option<String>,
    pub descendant_thread_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskUsageResponse {
    pub data_as_of: Option<String>,
    pub threads: Vec<TaskUsage>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskUsageStatus {
    Available,
    Partial,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskUsage {
    pub thread_id: String,
    pub data_status: TaskUsageStatus,
    pub usage_source: String,
    #[serde(flatten)]
    pub amounts: TaskUsageAmounts,
    pub groups: Vec<TaskUsageGroup>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskUsageAmounts {
    #[serde(default, deserialize_with = "percentage")]
    pub five_hour_limit_percent: Option<f64>,
    #[serde(default, deserialize_with = "percentage")]
    pub weekly_limit_percent: Option<f64>,
    pub balance_usage_credits: Option<TaskCredits>,
}

// Flattened Serde fields buffer arbitrary-precision JSON numbers as maps.
// Decode through Number so decimal percentages work with either serde_json configuration.
fn percentage<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Option::<serde_json::Number>::deserialize(deserializer)?
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| serde::de::Error::custom("Invalid task percentage."))
        })
        .transpose()
}

#[derive(Clone, Debug, Deserialize)]
pub struct TaskUsageGroup {
    pub product_experience: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub speed: Option<String>,
    #[serde(flatten)]
    pub amounts: TaskUsageAmounts,
}

impl Client {
    /// Queries disjoint tasks, including legacy descendants, without selecting a comparison plan.
    pub async fn get_task_usage(
        &self,
        threads: &[TaskUsageThread],
    ) -> Result<TaskUsageResponse, RequestError> {
        let mut ids = HashSet::new();
        if threads.is_empty()
            || threads.len() > 100
            || threads.iter().any(|thread| {
                std::iter::once(&thread.thread_id)
                    .chain(&thread.descendant_thread_ids)
                    .any(|id| id.trim().is_empty() || id.len() > 512 || !ids.insert(id.as_str()))
            })
            || ids.len() > 1_000
        {
            return Err(RequestError::Other(anyhow::anyhow!(
                "Expected at most 100 disjoint tasks and 1,000 thread IDs."
            )));
        }
        let prefix = match self.path_style {
            PathStyle::CodexApi => "api/codex",
            PathStyle::ChatGptApi => "wham",
        };
        let url = format!("{}/{prefix}/usage/thread_usage/query_v2", self.base_url);
        #[derive(Serialize)]
        struct Query<'a> {
            threads: &'a [TaskUsageThread],
        }
        let request = self
            .request(Method::POST, &url)
            .headers(self.headers())
            .json(&Query { threads });
        let (body, _) = self.exec_request_detailed(request, "POST", &url).await?;
        let response: TaskUsageResponse = serde_json::from_str(&body)
            .map_err(|_| RequestError::Other(anyhow::anyhow!("Invalid task usage response.")))?;
        let roots: HashSet<_> = threads
            .iter()
            .map(|thread| thread.thread_id.as_str())
            .collect();
        let mut seen = HashSet::new();
        if response.threads.iter().any(|thread| {
            !roots.contains(thread.thread_id.as_str())
                || !seen.insert(&thread.thread_id)
                || std::iter::once(&thread.amounts)
                    .chain(thread.groups.iter().map(|group| &group.amounts))
                    .any(|amounts| {
                        [
                            amounts.five_hour_limit_percent,
                            amounts.weekly_limit_percent,
                        ]
                        .into_iter()
                        .flatten()
                        .any(|value| !value.is_finite())
                    })
        }) {
            return Err(RequestError::Other(anyhow::anyhow!(
                "Invalid task usage response."
            )));
        }
        Ok(response)
    }
}

/// Decimal strings retain all backend digits for both rendering and sorting.
#[derive(Clone, Debug, Eq)]
pub struct TaskCredits(String);

impl TaskCredits {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parts(&self) -> (bool, i64, String) {
        let (mantissa, exponent) = self.0.split_once(['e', 'E']).unwrap_or((&self.0, "0"));
        let negative = mantissa.starts_with('-');
        let unsigned = mantissa.trim_start_matches(['-', '+']);
        let fraction = unsigned
            .split_once('.')
            .map_or(/*default*/ 0, |(_, fraction)| fraction.len());
        let digits = unsigned.replace('.', "");
        let digits = digits.trim_start_matches('0');
        let power =
            exponent.parse::<i64>().unwrap_or_default() - fraction as i64 + digits.len() as i64;
        (negative && !digits.is_empty(), power, digits.to_string())
    }
}

impl<'de> Deserialize<'de> for TaskCredits {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let mantissa = value
            .split(['e', 'E'])
            .next()
            .unwrap_or_default()
            .trim_start_matches(['-', '+']);
        let exponent = value
            .split_once(['e', 'E'])
            .map_or("0", |(_, exponent)| exponent);
        if value.len() > 128
            || mantissa.is_empty()
            || !mantissa.chars().any(|ch| ch.is_ascii_digit())
            || !mantissa.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
            || value.parse::<f64>().is_err()
            || exponent.parse::<i32>().is_err()
        {
            return Err(serde::de::Error::custom("Invalid decimal credits."));
        }
        Ok(Self(value))
    }
}

impl Ord for TaskCredits {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (negative, power, digits) = self.parts();
        let (other_negative, other_power, other_digits) = other.parts();
        let magnitude = match (digits.is_empty(), other_digits.is_empty()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => power.cmp(&other_power).then_with(|| {
                let width = digits.len().max(other_digits.len());
                digits
                    .bytes()
                    .chain(std::iter::repeat(b'0'))
                    .take(width)
                    .cmp(
                        other_digits
                            .bytes()
                            .chain(std::iter::repeat(b'0'))
                            .take(width),
                    )
            }),
        };
        other_negative.cmp(&negative).then(if negative {
            magnitude.reverse()
        } else {
            magnitude
        })
    }
}
impl PartialOrd for TaskCredits {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for TaskCredits {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

#[cfg(test)]
#[path = "task_usage_tests.rs"]
mod tests;
