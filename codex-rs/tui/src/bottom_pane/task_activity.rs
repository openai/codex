/// Tracks foreground work independently from MCP startup running in the background.
///
/// The aggregate busy state preserves the existing composer and spinner behavior, while the
/// individual states let command gating and status ownership distinguish the two lifecycles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TaskActivity {
    foreground_task_running: bool,
    mcp_startup_running: bool,
}

impl TaskActivity {
    pub(crate) fn is_busy(self) -> bool {
        self.foreground_task_running || self.mcp_startup_running
    }

    pub(crate) fn foreground_task_running(self) -> bool {
        self.foreground_task_running
    }

    pub(crate) fn mcp_startup_running(self) -> bool {
        self.mcp_startup_running
    }

    pub(crate) fn set_foreground_task_running(&mut self, running: bool) {
        self.foreground_task_running = running;
    }

    pub(crate) fn set_mcp_startup_running(&mut self, running: bool) {
        self.mcp_startup_running = running;
    }
}
