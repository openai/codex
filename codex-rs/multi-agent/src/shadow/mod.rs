mod recorder;

use std::collections::VecDeque;
use std::io::Write;
use std::time::SystemTime;

use anyhow::Context;
use anyhow::Result;
use bincode;
use codex_core::protocol::Event;
use codex_core::protocol::InputItem;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;

use recorder::ShadowRecorder;

use crate::AgentId;

pub use recorder::ShadowHistoryEntry;
pub use recorder::ShadowHistoryKind;
pub use recorder::ShadowSessionMetrics as ShadowMetrics;
use recorder::ShadowSessionMetrics;
pub use recorder::ShadowSnapshot;
pub use recorder::ShadowTranscriptCapture;

#[derive(Debug, Clone, Copy)]
pub struct ShadowConfig {
    pub enabled: bool,
    pub max_sessions: Option<usize>,
    pub max_memory_bytes: Option<usize>,
    pub compress: bool,
}

impl ShadowConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            max_sessions: None,
            max_memory_bytes: None,
            compress: false,
        }
    }

    pub fn apply_defaults(
        enabled: bool,
        max_sessions: Option<usize>,
        max_memory_bytes: Option<usize>,
        compress: bool,
    ) -> Self {
        Self {
            enabled,
            max_sessions,
            max_memory_bytes,
            compress,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShadowSessionSummary {
    pub metrics: ShadowSessionMetrics,
    pub raw_bytes: usize,
    pub compressed_bytes: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct ShadowData {
    recorder: ShadowRecorder,
    events: Vec<Event>,
}

impl ShadowData {
    fn new(conversation_id: &str, agent_id: &AgentId) -> Self {
        Self {
            recorder: ShadowRecorder::new(conversation_id.to_string(), agent_id.clone()),
            events: Vec::new(),
        }
    }

    fn record_event(&mut self, event: &Event) {
        self.recorder.record_event(event);
        self.events.push(event.clone());
    }

    fn record_user_inputs(&mut self, items: &[InputItem]) {
        for item in items {
            if let InputItem::Text { text } = item {
                if text.trim().is_empty() {
                    continue;
                }
                let event = self.recorder.make_user_event(text.clone());
                self.recorder.record_event(&event);
                self.events.push(event);
            }
        }
        self.recorder.record_user_inputs(items);
    }

    fn record_agent_outputs(&mut self, outputs: &[String]) {
        self.recorder.record_agent_outputs(outputs);
    }

    fn snapshot(&self) -> ShadowSnapshot {
        self.recorder.snapshot(&self.events)
    }

    fn metrics(&self) -> ShadowSessionMetrics {
        self.recorder.metrics()
    }

    fn raw_bytes(&self) -> usize {
        self.recorder.raw_bytes()
    }
}

enum ShadowStorage {
    Uncompressed(ShadowData),
    Compressed {
        bytes: Vec<u8>,
        raw_bytes: usize,
        metrics: ShadowSessionMetrics,
    },
}

impl ShadowStorage {
    fn new(conversation_id: &str, agent_id: &AgentId) -> Self {
        Self::Uncompressed(ShadowData::new(conversation_id, agent_id))
    }

    fn ensure_uncompressed(
        &mut self,
        conversation_id: &str,
        agent_id: &AgentId,
    ) -> Result<&mut ShadowData> {
        if let ShadowStorage::Compressed { bytes, .. } = self {
            let mut decoder = GzDecoder::new(bytes.as_slice());
            let restored: ShadowData =
                bincode::deserialize_from(&mut decoder).context("decompress shadow data")?;
            *self = ShadowStorage::Uncompressed(restored);
        }
        Ok(match self {
            ShadowStorage::Uncompressed(data) => data,
            ShadowStorage::Compressed { .. } => {
                *self = ShadowStorage::Uncompressed(ShadowData::new(conversation_id, agent_id));
                match self {
                    ShadowStorage::Uncompressed(data) => data,
                    _ => unreachable!(),
                }
            }
        })
    }

    fn compress(&mut self) -> Result<()> {
        if let ShadowStorage::Uncompressed(data) = self {
            let mut buf = Vec::new();
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            bincode::serialize_into(&mut buf, data).context("serialize shadow data")?;
            encoder
                .write_all(&buf)
                .context("compress serialized shadow data")?;
            let compressed = encoder.finish().context("finish compression")?;
            let metrics = data.metrics();
            let raw_bytes = data.raw_bytes().max(buf.len());
            *self = ShadowStorage::Compressed {
                bytes: compressed,
                raw_bytes,
                metrics,
            };
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<ShadowSnapshot> {
        match self {
            ShadowStorage::Uncompressed(data) => Ok(data.snapshot()),
            ShadowStorage::Compressed { bytes, .. } => {
                let mut decoder = GzDecoder::new(bytes.as_slice());
                let restored: ShadowData =
                    bincode::deserialize_from(&mut decoder).context("decompress shadow data")?;
                Ok(restored.snapshot())
            }
        }
    }

    fn raw_bytes(&self) -> usize {
        match self {
            ShadowStorage::Uncompressed(data) => data.raw_bytes(),
            ShadowStorage::Compressed { raw_bytes, .. } => *raw_bytes,
        }
    }

    fn compressed_bytes(&self) -> Option<usize> {
        match self {
            ShadowStorage::Compressed { bytes, .. } => Some(bytes.len()),
            _ => None,
        }
    }

    fn metrics(&self) -> ShadowSessionMetrics {
        match self {
            ShadowStorage::Uncompressed(data) => data.metrics(),
            ShadowStorage::Compressed { metrics, .. } => *metrics,
        }
    }
}

struct ShadowSession {
    agent_id: AgentId,
    storage: ShadowStorage,
    last_updated: SystemTime,
}

impl ShadowSession {
    fn new(conversation_id: &str, agent_id: AgentId) -> Self {
        Self {
            storage: ShadowStorage::new(conversation_id, &agent_id),
            agent_id,
            last_updated: SystemTime::now(),
        }
    }

    fn touch(&mut self) {
        self.last_updated = SystemTime::now();
    }
}

pub struct ShadowManager {
    config: ShadowConfig,
    sessions: Mutex<VecDeque<(String, ShadowSession)>>,
}

impl ShadowManager {
    pub fn new(config: ShadowConfig) -> Self {
        Self {
            config,
            sessions: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn register_session(&self, conversation_id: &str, agent_id: &AgentId) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let mut sessions = self.sessions.lock().await;
        if sessions
            .iter()
            .any(|(id, _)| id.as_str() == conversation_id)
        {
            return Ok(());
        }
        sessions.push_back((
            conversation_id.to_string(),
            ShadowSession::new(conversation_id, agent_id.clone()),
        ));
        drop(sessions);
        self.enforce_limits().await?;
        Ok(())
    }

    pub async fn record_event(
        &self,
        conversation_id: &str,
        agent_id: &AgentId,
        event: &Event,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let mut sessions = self.sessions.lock().await;
        if let Some((_, session)) = sessions
            .iter_mut()
            .find(|(id, _)| id.as_str() == conversation_id)
        {
            let data = session
                .storage
                .ensure_uncompressed(conversation_id, agent_id)?;
            data.record_event(event);
            session.touch();
            if self.config.compress {
                let _ = session.storage.compress();
            }
        } else {
            sessions.push_back((
                conversation_id.to_string(),
                ShadowSession::new(conversation_id, agent_id.clone()),
            ));
            if let Some((_, session)) = sessions
                .iter_mut()
                .find(|(id, _)| id.as_str() == conversation_id)
            {
                let data = session
                    .storage
                    .ensure_uncompressed(conversation_id, agent_id)?;
                data.record_event(event);
                session.touch();
                if self.config.compress {
                    let _ = session.storage.compress();
                }
            }
        }
        drop(sessions);
        self.enforce_limits().await?;
        Ok(())
    }

    pub async fn record_user_inputs(
        &self,
        conversation_id: &str,
        agent_id: &AgentId,
        inputs: &[InputItem],
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let mut sessions = self.sessions.lock().await;
        if let Some((_, session)) = sessions
            .iter_mut()
            .find(|(id, _)| id.as_str() == conversation_id)
        {
            let data = session
                .storage
                .ensure_uncompressed(conversation_id, agent_id)?;
            data.record_user_inputs(inputs);
            session.touch();
            if self.config.compress {
                let _ = session.storage.compress();
            }
        }
        Ok(())
    }

    pub async fn record_agent_outputs(
        &self,
        conversation_id: &str,
        agent_id: &AgentId,
        outputs: &[String],
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let mut sessions = self.sessions.lock().await;
        if let Some((_, session)) = sessions
            .iter_mut()
            .find(|(id, _)| id.as_str() == conversation_id)
        {
            let data = session
                .storage
                .ensure_uncompressed(conversation_id, agent_id)?;
            data.record_agent_outputs(outputs);
            session.touch();
            if self.config.compress {
                let _ = session.storage.compress();
            }
        }
        Ok(())
    }

    pub async fn snapshot(&self, conversation_id: &str) -> Option<ShadowSnapshot> {
        if !self.config.enabled {
            return None;
        }
        let sessions = self.sessions.lock().await;
        let (_, session) = sessions.iter().find(|(id, _)| id == conversation_id)?;
        session.storage.snapshot().ok()
    }

    pub async fn metrics(&self) -> ShadowSessionMetrics {
        if !self.config.enabled {
            return ShadowSessionMetrics::default();
        }
        let sessions = self.sessions.lock().await;
        let mut metrics = ShadowSessionMetrics::default();
        metrics.session_count = sessions.len();
        for (_, session) in sessions.iter() {
            let m = session.storage.metrics();
            metrics.events += m.events;
            metrics.user_inputs += m.user_inputs;
            metrics.agent_outputs += m.agent_outputs;
            metrics.turns += m.turns;
            metrics.total_bytes += session.storage.raw_bytes();
            if let Some(bytes) = session.storage.compressed_bytes() {
                metrics.total_compressed_bytes += bytes;
            }
        }
        metrics
    }

    pub async fn session_summary(&self, conversation_id: &str) -> Option<ShadowSessionSummary> {
        if !self.config.enabled {
            return None;
        }
        let sessions = self.sessions.lock().await;
        let (_, session) = sessions.iter().find(|(id, _)| id == conversation_id)?;
        Some(ShadowSessionSummary {
            metrics: session.storage.metrics(),
            raw_bytes: session.storage.raw_bytes(),
            compressed_bytes: session.storage.compressed_bytes(),
        })
    }

    pub async fn remove_session(&self, conversation_id: &str) {
        if !self.config.enabled {
            return;
        }
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|(id, _)| id != conversation_id);
    }

    pub async fn touch(&self, conversation_id: &str) {
        if !self.config.enabled {
            return;
        }
        let mut sessions = self.sessions.lock().await;
        if let Some((_, session)) = sessions
            .iter_mut()
            .find(|(id, _)| id.as_str() == conversation_id)
        {
            session.touch();
        }
    }

    async fn enforce_limits(&self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        loop {
            let mut sessions = self.sessions.lock().await;
            let over_session_limit = self
                .config
                .max_sessions
                .map(|max| sessions.len() > max)
                .unwrap_or(false);
            let total_bytes = sessions
                .iter()
                .map(|(_, session)| session.storage.raw_bytes())
                .sum::<usize>();
            let over_memory_limit = self
                .config
                .max_memory_bytes
                .map(|limit| total_bytes > limit)
                .unwrap_or(false);
            if !over_session_limit && !over_memory_limit {
                break;
            }
            if let Some((_, session)) = sessions.pop_front() {
                tracing::info!(
                    agent = %session.agent_id.as_str(),
                    "Shadow cache evicted for #{}",
                    session.agent_id.as_str()
                );
            } else {
                break;
            }
        }
        Ok(())
    }
}
