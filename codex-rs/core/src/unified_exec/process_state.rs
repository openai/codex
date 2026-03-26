#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProcessState {
    pub(crate) has_exited: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) failure_message: Option<String>,
}

impl ProcessState {
    pub(crate) fn exited(&self, exit_code: Option<i32>) -> Self {
        Self {
            has_exited: true,
            exit_code,
            failure_message: self.failure_message.clone(),
        }
    }

    pub(crate) fn failed(&self, message: String) -> Self {
        Self {
            has_exited: true,
            exit_code: self.exit_code,
            failure_message: Some(message),
        }
    }
}
