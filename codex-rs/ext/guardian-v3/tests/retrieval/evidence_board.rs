//! A Guardian board seeded with the same routed originals the housekeeper writes.

use crate::board_host::Host;
use crate::retrieval_host;
use codex_guardian_v3::EvidenceId;
use codex_guardian_v3::local_board;
use codex_history::RolloutItem;
use codex_message_board::AppendRequest;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::PostDestination;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_state::SqliteConfig;
use serde::Serialize;
use std::sync::Arc;
use tempfile::TempDir;

pub(super) struct EvidenceBoard {
    pub host: Arc<Host>,
    pub writer: LocalAgentMessageBoard,
    owner: ThreadId,
    _directory: TempDir,
}

impl EvidenceBoard {
    pub async fn new(
        owner: ThreadId,
        reviewer: ThreadId,
        channels: &[&str],
    ) -> anyhow::Result<Self> {
        let path = AgentPath::root()
            .join("reviewer")
            .map_err(anyhow::Error::msg)?;
        let host = retrieval_host([(owner, AgentPath::root()), (reviewer, path)]);
        let directory = tempfile::tempdir()?;
        let writer = local_board(
            SqliteConfig::new_for_testing(directory.path().to_path_buf().try_into()?),
            owner.into(),
        )
        .read_write()
        .open(host.clone())
        .await?;
        for &channel in channels {
            writer
                .upsert_channel(
                    owner,
                    UpsertChannelRequest {
                        channel_name: channel.into(),
                        description: String::new().try_into()?,
                    },
                )
                .await?;
        }
        Ok(Self {
            host,
            writer,
            owner,
            _directory: directory,
        })
    }

    pub async fn append(&self, ordinal: u64, channel: &str, output: &str) -> anyhow::Result<()> {
        self.writer
            .append(
                self.owner,
                AppendRequest {
                    request_id: serde_json::to_string(&(ordinal, channel))?,
                    destination: PostDestination::Channel(channel.into()),
                    text: serde_json::to_string(&routed_original(ordinal, output))?,
                },
            )
            .await?;
        Ok(())
    }
}

pub(super) fn routed_original(ordinal: u64, output: &str) -> impl Serialize {
    // Preserve the housekeeper's field order as well as the original wire contents.
    #[derive(Serialize)]
    struct RoutedSource {
        id: EvidenceId,
        original: RolloutItem,
    }
    RoutedSource {
        id: EvidenceId { ordinal },
        original: RolloutItem::ResponseItem(
            ResponseItem::FunctionCallOutput {
                id: None,
                call_id: None,
                name: None,
                namespace: None,
                output: FunctionCallOutputPayload::from_text(output.to_owned()),
                internal_chat_message_metadata_passthrough: None,
            }
            .into(),
        ),
    }
}
