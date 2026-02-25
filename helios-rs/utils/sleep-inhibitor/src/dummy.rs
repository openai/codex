#[derive(Debug, Default)]
pub(crate) struct SleepInhibitor;

impl SleepInhibitor {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn acquire(&mut self) {}

    pub(crate) fn release(&mut self) {}
}
