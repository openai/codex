use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::ExecServerError;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::WriteResponse;

pub(crate) const SESSION_EVENT_CHANNEL_CAPACITY: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecSessionEvent {
    Output {
        seq: u64,
        stream: ExecOutputStream,
        chunk: Vec<u8>,
    },
    Exited {
        seq: u64,
        exit_code: i32,
    },
    Closed {
        seq: u64,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessId(String);

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
    pub events: mpsc::Receiver<ExecSessionEvent>,
}

impl ProcessId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for ProcessId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ProcessId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for ProcessId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &ProcessId;

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError>;

    async fn terminate(&self) -> Result<(), ExecServerError>;
}

#[async_trait]
pub trait ExecBackend: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError>;
}
