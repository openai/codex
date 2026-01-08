#[cfg(feature = "entertainment")]
use std::time::Duration;
use std::time::Instant;

#[cfg(feature = "entertainment")]
use std::cell::Cell;
#[cfg(feature = "entertainment")]
use std::cell::RefCell;

#[cfg(feature = "entertainment")]
use crate::shimmer_text::ShimmerText;

#[cfg(feature = "entertainment")]
const SHIMMER_TEXT_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct RenderHeader {
    pub(crate) face: Option<String>,
    pub(crate) text: String,
}

#[cfg(feature = "entertainment")]
pub(crate) struct StatusShimmer {
    header: String,
    use_shimmer_text: bool,
    shimmer_text: RefCell<ShimmerText>,
    shimmer_face_cache: RefCell<String>,
    shimmer_text_cache: RefCell<String>,
    last_shimmer_update: Cell<Instant>,
    shimmer_interval: Cell<Duration>,
}

#[cfg(feature = "entertainment")]
impl StatusShimmer {
    pub(crate) fn new(now: Instant) -> Self {
        let mut shimmer_text = ShimmerText::new();
        let shimmer_step = shimmer_text.get_next();
        let shimmer_interval = shimmer_text.next_interval(SHIMMER_TEXT_INTERVAL);
        Self {
            header: shimmer_step.text.clone(),
            use_shimmer_text: true,
            shimmer_text: RefCell::new(shimmer_text),
            shimmer_face_cache: RefCell::new(shimmer_step.face),
            shimmer_text_cache: RefCell::new(shimmer_step.text),
            last_shimmer_update: Cell::new(now),
            shimmer_interval: Cell::new(shimmer_interval),
        }
    }

    pub(crate) fn update_header(&mut self, header: String) {
        let was_shimmer = self.use_shimmer_text;
        let use_shimmer = self.shimmer_text.borrow().is_default_label(&header);
        self.use_shimmer_text = use_shimmer;
        if use_shimmer {
            self.header = header.clone();
            if !was_shimmer {
                let next = self.shimmer_text.borrow_mut().reset_and_get_next();
                self.set_shimmer_step(next);
                let next_interval = self
                    .shimmer_text
                    .borrow_mut()
                    .next_interval(SHIMMER_TEXT_INTERVAL);
                self.shimmer_interval.set(next_interval);
            }
            self.last_shimmer_update.set(Instant::now());
        } else {
            self.header = header;
        }
    }

    #[cfg(test)]
    pub(crate) fn header_for_test(&self) -> String {
        if self.use_shimmer_text {
            self.shimmer_text_cache.borrow().clone()
        } else {
            self.header.clone()
        }
    }

    pub(crate) fn render_header(&self, now: Instant) -> RenderHeader {
        if !self.use_shimmer_text {
            return RenderHeader {
                face: Some(self.shimmer_face_cache.borrow().clone()),
                text: self.header.clone(),
            };
        }

        let elapsed = now.saturating_duration_since(self.last_shimmer_update.get());
        if elapsed >= self.shimmer_interval.get() {
            let next = self.shimmer_text.borrow_mut().get_next();
            self.set_shimmer_step(next);
            self.last_shimmer_update.set(now);
            let next_interval = self
                .shimmer_text
                .borrow_mut()
                .next_interval(SHIMMER_TEXT_INTERVAL);
            self.shimmer_interval.set(next_interval);
        }

        RenderHeader {
            face: Some(self.shimmer_face_cache.borrow().clone()),
            text: self.shimmer_text_cache.borrow().clone(),
        }
    }

    fn set_shimmer_step(&self, step: crate::shimmer_text::ShimmerStep) {
        *self.shimmer_face_cache.borrow_mut() = step.face;
        *self.shimmer_text_cache.borrow_mut() = step.text;
    }
}

#[cfg(not(feature = "entertainment"))]
pub(crate) struct StatusShimmer {
    header: String,
}

#[cfg(not(feature = "entertainment"))]
impl StatusShimmer {
    pub(crate) fn new(_now: Instant) -> Self {
        Self {
            header: String::from("Working"),
        }
    }

    pub(crate) fn update_header(&mut self, header: String) {
        self.header = header;
    }

    #[cfg(test)]
    pub(crate) fn header_for_test(&self) -> String {
        self.header.clone()
    }

    pub(crate) fn render_header(&self, _now: Instant) -> RenderHeader {
        RenderHeader {
            face: None,
            text: self.header.clone(),
        }
    }
}
