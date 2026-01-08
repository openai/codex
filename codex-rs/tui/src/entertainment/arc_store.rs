use crate::status_indicator_widget::StatusIndicatorWidget;

#[derive(Debug)]
pub(crate) struct EntertainmentArcStore {
    enabled: bool,
    arcs: Vec<Vec<String>>,
}

impl EntertainmentArcStore {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            arcs: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, texts: Vec<String>, status: Option<&mut StatusIndicatorWidget>) {
        if !self.enabled || texts.is_empty() {
            return;
        }
        self.arcs.push(texts.clone());
        if let Some(status) = status {
            status.add_entertainment_arc(texts);
        }
    }

    pub(crate) fn apply_to(&self, status: &mut StatusIndicatorWidget) {
        if !self.enabled || self.arcs.is_empty() {
            return;
        }
        status.set_entertainment_arcs(self.arcs.clone());
    }
}
