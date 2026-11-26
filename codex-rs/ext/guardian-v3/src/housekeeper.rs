//! Serial routing and description updates over the existing board operations.
//! Batches keep whole originals within the model's tool-output and context budgets.
//! Progress is process-local. Any error stops the pass; partial writes are not repaired.

use crate::EvidenceCursor;
use crate::EvidenceSnapshot;
use crate::OriginalEvidence;
use crate::housekeeper_sampler::HousekeeperSampler;
use crate::inbox::LiveDelivery;
use crate::sampling::context_input;
use anyhow::ensure;
use codex_core::CodexThread;
use codex_features::GuardianV3HousekeeperConfigToml;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::run_before_review_deadline;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::ChannelDescription;
use codex_message_board::ChannelQuery;
use codex_message_board::PageRequest;
use codex_message_board::PostDestination;
use codex_message_board::SortDirection;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_utils_string::to_json_string_bounded;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Serialize)]
struct RoutingInput<'a> {
    catalog: &'a BTreeMap<String, Option<String>>,
    sources: &'a [OriginalEvidence],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoutingOutput {
    routes: Vec<Vec<String>>,
    descriptions: Vec<DescriptionUpdate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptionUpdate {
    channel: String,
    description: ChannelDescription,
}

/// One serial housekeeper for an append-only root history and its private board.
/// The worker owns the model conversation. Its tree retains progress across worker replacement.
pub struct Housekeeper {
    board: Arc<dyn AgentMessageBoard>,
    owner: ThreadId,
    sampler: HousekeeperSampler,
    batch_timeout: Duration,
    stopped: bool,
    pub(super) shadow_size: Option<crate::shadow_metrics::BoardSize>,
    delivery: Arc<LiveDelivery>,
}

impl Housekeeper {
    /// Opens a separate model conversation for this board and its root's evidence cursor.
    pub async fn new(
        board: Arc<dyn AgentMessageBoard>,
        delivery: Arc<LiveDelivery>,
        parent: &Arc<CodexThread>,
        manager: &codex_core::ThreadManager,
        settings: &GuardianV3HousekeeperConfigToml,
    ) -> anyhow::Result<Self> {
        let owner = delivery.root;
        ensure!(
            board.identity() == SessionId::from(owner)
                && parent.startup_metadata().thread_id == owner,
            "housekeeper root mismatch"
        );
        Ok(Self {
            board,
            owner,
            sampler: HousekeeperSampler::new(
                parent,
                manager,
                settings.routing_instructions.as_deref(),
            )
            .await?,
            batch_timeout: settings
                .batch_timeout_ms
                .map_or(Duration::from_secs(90), |ms| {
                    Duration::from_millis(u64::from(ms.get()))
                }),
            stopped: false,
            shadow_size: None,
            delivery,
        })
    }

    /// Use this position to capture the next unread range. Reads do not advance it.
    pub fn cursor(&self) -> EvidenceCursor {
        self.delivery.cursor()
    }

    /// Route one batch from this fixed capture and refresh its channel descriptions
    /// in the same model call. Returns the number of newly routed originals; zero
    /// means this capture is complete. Await completion before using its channel state.
    pub async fn route_next(
        &mut self,
        snapshot: &EvidenceSnapshot,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<usize> {
        // Dropping a routing future can interrupt a batch after its first board write.
        ensure!(!self.stopped, "housekeeper stopped after an earlier error");
        self.stopped = true;
        let deadline = ReviewDeadline::from_timeout(Some(self.batch_timeout));
        let active = cancellation.child_token();
        let account_stop = CancellationToken::new();
        let account_changed = self.sampler.account_changed();
        let cancellation = &active;
        let routing = async {
            check_active(deadline, cancellation)?;
            ensure!(
                snapshot.thread_id() == self.owner,
                "housekeeper snapshot owner mismatch"
            );
            let pending = self.cursor().pending(snapshot)?;
            if pending.is_empty() {
                // Advance past records intentionally excluded from evidence selection.
                self.delivery.acknowledge(snapshot, /*count*/ 0)?;
                return Ok(0);
            }
            let mut catalog = BTreeMap::new();
            let mut cursor = None;
            loop {
                check_active(deadline, cancellation)?;
                let listing = self.board.list_channels(
                    self.owner,
                    ChannelQuery {
                        query: None,
                        direction: SortDirection::OldestFirst,
                        page: PageRequest {
                            cursor,
                            limit: NonZeroU32::new(50).unwrap_or(NonZeroU32::MIN),
                        },
                    },
                );
                let page = run_before_review_deadline(deadline, Some(cancellation), listing)
                    .await
                    .map_err(|reason| {
                        anyhow::anyhow!("listing housekeeper channels: {reason:?}")
                    })??;
                for channel in page.channels.results {
                    catalog.insert(channel.channel_name, channel.description);
                }
                cursor = page.channels.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            let policy = self.sampler.input_policy();
            let input_budget = policy.byte_budget();
            let empty_input = RoutingInput {
                catalog: &catalog,
                sources: &[],
            };
            // The empty input includes the catalog and array delimiters. Each original
            // adds its serialized bytes and, after the first, one separating comma.
            let mut input_bytes = to_json_string_bounded(&empty_input, input_budget)?.len();
            let mut texts = Vec::new();
            for source in pending {
                // Preserve the existing board's post-size limit, including source metadata.
                let text = to_json_string_bounded(source, 64 * 1024)?;
                // Every valid batch must route the first original to at least one channel.
                if texts.is_empty()
                    && let Some(size) = self.shadow_size
                {
                    size.check_capacity(text.len() as u64)?;
                }
                let added_bytes = text.len() + usize::from(!texts.is_empty());
                if added_bytes > input_budget.saturating_sub(input_bytes) {
                    break;
                }
                input_bytes += added_bytes;
                texts.push(text);
            }
            ensure!(
                !texts.is_empty(),
                "housekeeper catalog and original exceed the model's tool-output budget"
            );
            let (source_count, output, batch) = run_before_review_deadline(
                deadline,
                Some(cancellation),
                self.sampler.sample(texts.len(), |count| {
                    let input = RoutingInput {
                        catalog: &catalog,
                        sources: &pending[..count],
                    };
                    let item = context_input("guardian_housekeeper_input", &input, policy)?;
                    Ok((item, routing_schema(count)))
                }),
            )
            .await
            .map_err(|reason| anyhow::anyhow!("housekeeper sampling: {reason:?}"))??;
            let sources = &pending[..source_count];
            let update_count = output.descriptions.len();
            let descriptions: BTreeMap<_, _> = output
                .descriptions
                .into_iter()
                .map(|update| (update.channel, update.description))
                .collect();
            let valid = descriptions.len() == update_count
                && output.routes.len() == sources.len()
                && output.routes.iter().all(|route| {
                    !route.is_empty()
                        && route.iter().all(|name| {
                            catalog.contains_key(name) || descriptions.contains_key(name)
                        })
                })
                && descriptions
                    .keys()
                    .all(|name| output.routes.iter().any(|route| route.contains(name)));
            ensure!(
                valid,
                "housekeeper returned invalid source routes or descriptions"
            );
            let next_size = self
                .shadow_size
                .map(|size| {
                    let added = descriptions
                        .keys()
                        .filter(|name| !catalog.contains_key(*name))
                        .count();
                    size.with_batch(catalog.len() + added, &output.routes, &texts)
                })
                .transpose()?;
            for (channel_name, description) in descriptions {
                if catalog.get(&channel_name).and_then(Option::as_deref)
                    == Some(description.as_str())
                {
                    continue;
                }
                check_active(deadline, cancellation)?;
                let upsert = self.board.upsert_channel(
                    self.owner,
                    UpsertChannelRequest {
                        channel_name,
                        description,
                    },
                );
                run_before_review_deadline(deadline, Some(cancellation), upsert)
                    .await
                    .map_err(|reason| {
                        anyhow::anyhow!("upserting housekeeper channel: {reason:?}")
                    })??;
            }
            for ((source, destinations), mut text) in sources.iter().zip(output.routes).zip(texts) {
                // Drain this original on graceful unload. Account changes and the batch
                // deadline still apply to each destination.
                check_active(deadline, cancellation)?;
                let mut remaining: BTreeSet<_> = destinations.iter().collect();
                for channel in &destinations {
                    if !remaining.remove(channel) {
                        continue;
                    }
                    let request = AppendRequest {
                        request_id: source.id.request_id(channel)?,
                        destination: PostDestination::Channel(channel.clone()),
                        // Multiple destinations need copies; the last one can take ownership.
                        text: if remaining.is_empty() {
                            std::mem::take(&mut text)
                        } else {
                            text.clone()
                        },
                    };
                    self.delivery
                        .append(
                            self.board.as_ref(),
                            request,
                            source.id,
                            deadline,
                            &account_stop,
                        )
                        .await?;
                }
                self.delivery.acknowledge(snapshot, /*count*/ 1)?;
            }
            self.shadow_size = next_size;
            self.sampler.commit(batch);
            Ok(sources.len())
        };
        let result = {
            tokio::pin!(routing);
            tokio::select! {
                biased;
                () = account_changed => {
                    // Let an already accepted append finish after the account changes.
                    active.cancel();
                    account_stop.cancel();
                    routing.await
                }
                result = &mut routing => result,
            }
        };
        self.stopped = result.is_err();
        result
    }
}

fn routing_schema(source_count: usize) -> serde_json::Value {
    json!({
        "type":"object","additionalProperties":false,"required":["routes","descriptions"],
        "properties":{
            "routes":{"type":"array","minItems":source_count,"maxItems":source_count,
                "items":{"type":"array","description":"Distinct destinations for this one original.","minItems":1,
                    "items":{"type":"string","description":"Channel name, 1–128 UTF-8 bytes without edge whitespace or control characters."}}},
            "descriptions":{"type":"array","description":"New or changed descriptions for channels used in routes.",
                "items":{"type":"object","additionalProperties":false,"required":["channel","description"],
                    "properties":{"channel":{"type":"string"},
                        "description":{"type":"string","description":"Channel purpose, at most 512 UTF-8 bytes."}}}}
        }
    })
}

pub(super) fn check_active(
    deadline: ReviewDeadline,
    cancellation: &CancellationToken,
) -> anyhow::Result<()> {
    ensure!(!cancellation.is_cancelled(), "housekeeping cancelled");
    ensure!(
        !matches!(deadline, ReviewDeadline::At(at) if Instant::now() >= at),
        "housekeeping deadline expired"
    );
    Ok(())
}
