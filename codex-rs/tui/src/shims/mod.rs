use codex_core::protocol::Op;
use crossterm::event::KeyEvent;
use ratatui::text::Line;

use crate::app_event::AppEvent;

pub mod thread;
// title_summary shim removed in favor of tool-driven title updates.
pub mod title_tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    Continue,
    Consumed,
}

/// Minimal host API surface available to shims for multi-session features.
pub trait HostApi {
    fn session_count(&self) -> usize;
    fn active_index(&self) -> usize;
    fn switch_next(&mut self) -> bool;
    fn switch_prev(&mut self) -> bool;
    fn active_display_name(&self) -> String;
    fn active_title(&self) -> Option<String>;
    fn set_active_title(&mut self, title: String);
    fn session_parent_index(&self, idx: usize) -> Option<usize>;
    fn session_title_at(&self, idx: usize) -> Option<String>;
    /// Submit an Op to the active session. Default no-op for hosts that do not support it.
    #[allow(dead_code)]
    fn submit_op(&mut self, _op: Op) {}
}

pub trait ShimModule: Send {
    fn on_app_event(&mut self, _event: &mut AppEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn on_app_event_with_host(
        &mut self,
        event: &mut AppEvent,
        _host: &mut dyn HostApi,
    ) -> EventOutcome {
        self.on_app_event(event)
    }

    fn on_key_event(&mut self, _event: &KeyEvent) -> EventOutcome {
        EventOutcome::Continue
    }

    fn on_key_event_with_host(
        &mut self,
        event: &KeyEvent,
        _host: &mut dyn HostApi,
    ) -> EventOutcome {
        self.on_key_event(event)
    }

    fn augment_header(&self, _lines: &mut Vec<Line<'static>>) {}

    fn augment_status_line(&self, _line: &mut Option<Line<'static>>) {}

    fn augment_header_with_host(&self, lines: &mut Vec<Line<'static>>, _host: &dyn HostApi) {
        self.augment_header(lines)
    }

    fn augment_status_line_with_host(&self, line: &mut Option<Line<'static>>, _host: &dyn HostApi) {
        self.augment_status_line(line)
    }
}

#[derive(Default)]
pub struct ShimStack {
    modules: Vec<Box<dyn ShimModule>>,
}

impl ShimStack {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn push<M>(&mut self, module: M)
    where
        M: ShimModule + 'static,
    {
        self.modules.push(Box::new(module));
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    #[allow(dead_code)]
    pub fn on_app_event(&mut self, event: &mut AppEvent) -> EventOutcome {
        for module in &mut self.modules {
            if module.on_app_event(event) == EventOutcome::Consumed {
                return EventOutcome::Consumed;
            }
        }
        EventOutcome::Continue
    }

    pub fn on_app_event_with_host(
        &mut self,
        event: &mut AppEvent,
        host: &mut dyn HostApi,
    ) -> EventOutcome {
        for module in &mut self.modules {
            if module.on_app_event_with_host(event, host) == EventOutcome::Consumed {
                return EventOutcome::Consumed;
            }
        }
        EventOutcome::Continue
    }

    #[allow(dead_code)]
    pub fn on_key_event(&mut self, event: &KeyEvent) -> EventOutcome {
        for module in &mut self.modules {
            if module.on_key_event(event) == EventOutcome::Consumed {
                return EventOutcome::Consumed;
            }
        }
        EventOutcome::Continue
    }

    pub fn on_key_event_with_host(
        &mut self,
        event: &KeyEvent,
        host: &mut dyn HostApi,
    ) -> EventOutcome {
        for module in &mut self.modules {
            if module.on_key_event_with_host(event, host) == EventOutcome::Consumed {
                return EventOutcome::Consumed;
            }
        }
        EventOutcome::Continue
    }

    #[allow(dead_code)]
    pub fn augment_header(&self, lines: &mut Vec<Line<'static>>) {
        for module in &self.modules {
            module.augment_header(lines);
        }
    }

    pub fn augment_header_with_host(&self, lines: &mut Vec<Line<'static>>, host: &dyn HostApi) {
        for module in &self.modules {
            module.augment_header_with_host(lines, host);
        }
    }

    #[allow(dead_code)]
    pub fn augment_status_line(&self, line: &mut Option<Line<'static>>) {
        for module in &self.modules {
            module.augment_status_line(line);
        }
    }

    pub fn augment_status_line_with_host(
        &self,
        line: &mut Option<Line<'static>>,
        host: &dyn HostApi,
    ) {
        for module in &self.modules {
            module.augment_status_line_with_host(line, host);
        }
    }
}
