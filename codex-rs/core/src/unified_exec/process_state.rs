//! Exit, failure, and sandbox-denial facts shared by process lifecycle observers.
//! Apply transitions under the watch channel's write lock to preserve concurrent updates.

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ProcessState {
    pub(crate) has_exited: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) failure_message: Option<String>,
    pub(crate) sandbox_denied: bool,
}

impl ProcessState {
    pub(crate) fn exited(&mut self, exit_code: Option<i32>) {
        self.has_exited = true;
        self.exit_code = exit_code;
    }

    pub(crate) fn failed(&mut self, message: String) {
        self.has_exited = true;
        self.failure_message = Some(message);
    }
}
