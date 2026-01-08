use std::cell::Cell;
use std::cell::RefCell;
use std::time::Duration;
use std::time::Instant;

use crate::shimmer_text::ShimmerText;

const SHIMMER_TEXT_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct RenderHeader {
    pub(crate) face: Option<String>,
    pub(crate) text: String,
}

pub(crate) struct StatusShimmer {
    entertainment_enabled: bool,
    header: String,
    entertainment: Option<EntertainmentState>,
}

struct EntertainmentState {
    use_shimmer_text: Cell<bool>,
    shimmer_text: RefCell<ShimmerText>,
    shimmer_face_cache: RefCell<String>,
    shimmer_text_cache: RefCell<String>,
    last_shimmer_update: Cell<Instant>,
    shimmer_interval: Cell<Duration>,
}

impl StatusShimmer {
    pub(crate) fn new(now: Instant, entertainment_enabled: bool) -> Self {
        if entertainment_enabled {
            let mut shimmer_text = ShimmerText::new();
            let shimmer_step = shimmer_text.get_next();
            let shimmer_interval = shimmer_text.next_interval(SHIMMER_TEXT_INTERVAL);
            let entertainment = EntertainmentState {
                use_shimmer_text: Cell::new(true),
                shimmer_text: RefCell::new(shimmer_text),
                shimmer_face_cache: RefCell::new(shimmer_step.face),
                shimmer_text_cache: RefCell::new(shimmer_step.text.clone()),
                last_shimmer_update: Cell::new(now),
                shimmer_interval: Cell::new(shimmer_interval),
            };
            Self {
                entertainment_enabled,
                header: shimmer_step.text,
                entertainment: Some(entertainment),
            }
        } else {
            Self {
                entertainment_enabled,
                header: String::from("Working"),
                entertainment: None,
            }
        }
    }

    pub(crate) fn update_header(&mut self, header: String) {
        self.header = header;
        if !self.entertainment_enabled {
            return;
        }
        let Some(state) = self.entertainment.as_ref() else {
            return;
        };
        let was_shimmer = state.use_shimmer_text.get();
        let use_shimmer = state.shimmer_text.borrow().is_default_label(&self.header);
        state.use_shimmer_text.set(use_shimmer);
        if use_shimmer {
            if !was_shimmer {
                let next = state.shimmer_text.borrow_mut().reset_and_get_next();
                self.set_shimmer_step(state, next);
                let next_interval = state
                    .shimmer_text
                    .borrow_mut()
                    .next_interval(SHIMMER_TEXT_INTERVAL);
                state.shimmer_interval.set(next_interval);
            }
            state.last_shimmer_update.set(Instant::now());
        }
    }

    pub(crate) fn add_entertainment_arc(&mut self, arc: Vec<String>) {
        if !self.entertainment_enabled {
            return;
        }
        let Some(state) = self.entertainment.as_ref() else {
            return;
        };
        state.shimmer_text.borrow_mut().add_generated_arc(arc);
        if state.use_shimmer_text.get() {
            let next = state.shimmer_text.borrow_mut().reset_and_get_next();
            self.set_shimmer_step(state, next);
            let next_interval = state
                .shimmer_text
                .borrow_mut()
                .next_interval(SHIMMER_TEXT_INTERVAL);
            state.shimmer_interval.set(next_interval);
            state.last_shimmer_update.set(Instant::now());
        }
    }

    pub(crate) fn set_entertainment_arcs(&mut self, arcs: Vec<Vec<String>>) {
        if !self.entertainment_enabled {
            return;
        }
        let Some(state) = self.entertainment.as_ref() else {
            return;
        };
        state.shimmer_text.borrow_mut().set_generated_arcs(arcs);
        if state.use_shimmer_text.get() {
            let next = state.shimmer_text.borrow_mut().reset_and_get_next();
            self.set_shimmer_step(state, next);
            let next_interval = state
                .shimmer_text
                .borrow_mut()
                .next_interval(SHIMMER_TEXT_INTERVAL);
            state.shimmer_interval.set(next_interval);
            state.last_shimmer_update.set(Instant::now());
        }
    }

    #[cfg(test)]
    pub(crate) fn header_for_test(&self) -> String {
        if let Some(state) = self.entertainment.as_ref()
            && state.use_shimmer_text.get()
        {
            return state.shimmer_text_cache.borrow().clone();
        }
        self.header.clone()
    }

    pub(crate) fn render_header(&self, now: Instant) -> RenderHeader {
        let Some(state) = self.entertainment.as_ref() else {
            return RenderHeader {
                face: None,
                text: self.header.clone(),
            };
        };
        if !state.use_shimmer_text.get() {
            return RenderHeader {
                face: Some(state.shimmer_face_cache.borrow().clone()),
                text: self.header.clone(),
            };
        }

        let elapsed = now.saturating_duration_since(state.last_shimmer_update.get());
        if elapsed >= state.shimmer_interval.get() {
            let next = state.shimmer_text.borrow_mut().get_next();
            self.set_shimmer_step(state, next);
            state.last_shimmer_update.set(now);
            let next_interval = state
                .shimmer_text
                .borrow_mut()
                .next_interval(SHIMMER_TEXT_INTERVAL);
            state.shimmer_interval.set(next_interval);
        }

        RenderHeader {
            face: Some(state.shimmer_face_cache.borrow().clone()),
            text: state.shimmer_text_cache.borrow().clone(),
        }
    }

    fn set_shimmer_step(&self, state: &EntertainmentState, step: crate::shimmer_text::ShimmerStep) {
        *state.shimmer_face_cache.borrow_mut() = step.face;
        *state.shimmer_text_cache.borrow_mut() = step.text;
    }
}
