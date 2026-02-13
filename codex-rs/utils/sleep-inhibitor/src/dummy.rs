use crate::PlatformSleepInhibitor;

#[derive(Debug, Default)]
pub(crate) struct DummySleepInhibitor;

impl DummySleepInhibitor {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl PlatformSleepInhibitor for DummySleepInhibitor {
    fn acquire(&mut self) {}

    fn release(&mut self) {}
}
