//! Historical allowance snapshots, separate from daily usage and current chat allowances.
//! Period IDs are snapshot-scoped; unknown accounting is never converted to zero.
use super::data::Load;
use chrono::DateTime;
use chrono::Utc;
use codex_backend_client::PlanLimitBreakdown;
use codex_backend_client::PlanLimitHistory;

pub(super) struct Period {
    pub id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub complete: bool,
    pub used: Option<f64>,
    pub breakdowns: Option<Vec<PlanLimitBreakdown>>,
}

pub(super) struct Report {
    pub as_of: DateTime<Utc>,
    pub coverage_start: Option<DateTime<Utc>>,
    pub coverage_complete: bool,
    pub approximate: bool,
    pub periods: [Vec<Period>; 2],
}

impl Report {
    pub(super) fn parse(history: PlanLimitHistory) -> Result<Option<Self>, String> {
        let parse = |value: &str| {
            DateTime::parse_from_rfc3339(value)
                .map(|date| date.with_timezone(&Utc))
                .map_err(|_| "Invalid plan history timestamp.".to_string())
        };
        let Some(as_of) = history.data_as_of.as_deref().map(parse).transpose()? else {
            return Ok(None);
        };
        let mut periods = [Vec::new(), Vec::new()];
        let mut ids = std::collections::HashSet::new();
        if history.periods.len() > 1_000 {
            return Err("Plan history contains too many periods.".into());
        }
        for period in history.periods {
            let window = match period.window_minutes {
                300 => 0,
                10080 => 1,
                _ => return Err("Unsupported plan history window.".into()),
            };
            let start = parse(&period.starts_at)?;
            let end = parse(&period.ends_at)?;
            if start >= end
                || !ids.insert(period.id.clone())
                || period
                    .used_basis_points
                    .is_some_and(|amount| !amount.is_finite())
                || period
                    .breakdowns
                    .iter()
                    .flatten()
                    .flat_map(|group| &group.rows)
                    .any(|row| !row.basis_points.is_finite())
            {
                return Err("Invalid plan history period.".into());
            }
            periods[window].push(Period {
                id: period.id,
                start,
                end,
                complete: period.accounting_complete,
                used: period.used_basis_points,
                breakdowns: period.breakdowns,
            });
        }
        for window in &mut periods {
            window.sort_by_key(|period| std::cmp::Reverse(period.start));
        }
        Ok(Some(Self {
            as_of,
            coverage_start: history.coverage_start.as_deref().map(parse).transpose()?,
            coverage_complete: history.coverage_complete,
            approximate: history.approximate,
            periods,
        }))
    }
}

pub(super) struct State {
    pub enabled: bool,
    pub report: Load<Report>,
    pub window: usize,
    pub cursor: [usize; 2],
    pub expanded: [Option<String>; 2],
    snapshot: Option<DateTime<Utc>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            report: Load::Unavailable,
            window: 0,
            cursor: [0; 2],
            expanded: [None, None],
            snapshot: None,
        }
    }
}

impl State {
    pub(super) fn poll(&mut self) {
        self.report.poll();
        let snapshot = self.report.ready().map(|report| report.as_of);
        if snapshot != self.snapshot {
            self.snapshot = snapshot;
            self.cursor = [0; 2];
            self.expanded = [None, None];
        }
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
