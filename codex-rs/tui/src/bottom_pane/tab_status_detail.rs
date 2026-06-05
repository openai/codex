use super::BottomPane;
use super::BottomPaneView;
use crate::tab_status::TabStatus;

impl BottomPane {
    pub(super) fn desired_tab_status(&self) -> (TabStatus, Option<String>) {
        let class = self.desired_tab_class();
        let detail = match class {
            TabStatus::Waiting => self
                .active_view()
                .and_then(BottomPaneView::tab_status_detail),
            TabStatus::Working => self.working_tab_status_detail(),
            TabStatus::Idle => self
                .tab_status
                .last_activity()
                .map(|activity| format!("last: {activity}")),
        };
        (class, detail)
    }

    fn working_tab_status_detail(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(activity) = self.tab_status.current_activity() {
            parts.push(activity.to_string());
        }
        let push_if_new = |parts: &mut Vec<String>, candidate: &str| {
            let candidate = candidate.trim();
            if candidate.is_empty()
                || parts
                    .iter()
                    .any(|part| part.contains(candidate) || candidate.contains(part.as_str()))
            {
                return;
            }
            parts.push(candidate.to_string());
        };
        if let Some(status) = self.status.as_ref() {
            let header = status.header();
            if header != "Working" {
                push_if_new(&mut parts, header);
            }
            if let Some(first_line) = status.details().and_then(|details| details.lines().next()) {
                push_if_new(&mut parts, first_line);
            }
        }
        if !parts.is_empty() {
            return Some(parts.join(" • "));
        }
        self.tab_status
            .last_activity()
            .map(|activity| format!("after: {activity}"))
    }
}
