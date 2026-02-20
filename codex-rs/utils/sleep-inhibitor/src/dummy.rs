#[derive(Debug, Default)]
pub(crate) struct SleepInhibitor;
use crate::PlatformSleepInhibitor;

impl SleepInhibitor {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl PlatformSleepInhibitor for SleepInhibitor {
    fn acquire(&mut self) {}

    fn release(&mut self) {}
}
