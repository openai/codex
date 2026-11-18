//! One tool-free inference client and append-only conversation per live root worker.
//! Under window pressure only whole, completed routing batches are evicted.
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use anyhow::ensure;
use codex_api::Reasoning;
use codex_api::ReasoningContext;
use codex_api::ResponseEvent;
use codex_api::ResponsesApiRequest;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::message_from_parts;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_guardian_context::effective_input_token_limit;
use codex_guardian_context::estimate_input_tokens;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::CatalogModel;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::TruncationPolicy;
use serde_json::Value;

use crate::HOUSEKEEPER_INSTRUCTIONS;
use crate::housekeeper::RoutingOutput;
use crate::housekeeper_transport::PendingResponse;
use crate::housekeeper_transport::Transport;

pub(super) struct HousekeeperSampler {
    parent: Weak<CodexThread>,
    conversation: ThreadId,
    transport: Transport,
    model: CatalogModel,
    openai_provider: bool,
    service_tier: Option<String>,
    instructions: ResponseItem,
    history: Vec<ResponseItem>,
    input_limit: usize,
    batches: VecDeque<RetainedBatch>,
    retained_tokens: usize,
}

struct RetainedBatch {
    items: usize,
    tokens: usize,
}

pub(super) struct SampledBatch {
    input_tokens: usize,
    output: Vec<ResponseItem>,
}

impl HousekeeperSampler {
    pub(super) async fn new(
        parent: &Arc<CodexThread>,
        manager: &ThreadManager,
    ) -> anyhow::Result<Self> {
        let config = parent.config().await;
        let snapshot = parent.config_snapshot().await;
        let mut settings = config.to_models_manager_config();
        // Actor window overrides apply only to the actor model.
        settings.model_context_window = None;
        settings.model_auto_compact_token_limit = None;
        let model = manager
            .models_manager_for_config(&config)
            .get_model_info("gpt-5.6-luna", &settings)
            .await;
        let input_limit = effective_input_token_limit(&model, /*configured_window*/ None);
        let mut instructions = message_from_parts(
            "developer",
            vec![AnnotatedContent::text(
                HOUSEKEEPER_INSTRUCTIONS,
                ContentItemKind("model.base_instructions".to_owned()),
                ContentItemMetadata::harness(),
            )],
        );
        if let Some(prefix) = instructions.id_prefix() {
            instructions.set_id(Some((parent.item_id_generator())(prefix)));
        }
        Ok(Self {
            parent: Arc::downgrade(parent),
            conversation: ThreadId::new(),
            transport: Transport::new(parent, &config, snapshot.originator)?,
            model,
            openai_provider: config.model_provider.is_openai(),
            service_tier: snapshot.service_tier,
            instructions,
            history: Vec::new(),
            input_limit,
            batches: VecDeque::new(),
            retained_tokens: 0,
        })
    }

    pub(super) fn account_changed(&self) -> impl std::future::Future<Output = ()> + use<> {
        let owner = self.transport.owner.clone();
        async move {
            let Some(owner) = owner else {
                return std::future::pending::<()>().await;
            };
            let mut changes = owner.changes();
            while owner.is_current() {
                if changes.changed().await.is_err() {
                    break;
                }
            }
        }
    }

    pub(super) fn input_policy(&self) -> TruncationPolicy {
        self.model.truncation_policy.into()
    }

    pub(super) async fn sample(
        &mut self,
        mut input: ResponseItem,
        schema: Value,
    ) -> anyhow::Result<(RoutingOutput, SampledBatch)> {
        let parent = self.parent.upgrade().context("Guardian root stopped")?;
        if let Some(prefix) = input.id_prefix() {
            input.set_id(Some((parent.item_id_generator())(prefix)));
        }
        drop(parent);
        let fixed_tokens = TruncationPolicy::Bytes(
            HOUSEKEEPER_INSTRUCTIONS.len() + serde_json::to_vec(&schema)?.len(),
        )
        .token_budget();
        let input_tokens = estimate_input_tokens(&input);
        let required = fixed_tokens.saturating_add(input_tokens);
        ensure!(
            required <= self.input_limit,
            "housekeeper batch exceeds model input window"
        );
        let mut remove = 0;
        while self.retained_tokens > self.input_limit - required {
            let batch = self
                .batches
                .pop_front()
                .context("inconsistent housekeeper history")?;
            self.retained_tokens -= batch.tokens;
            remove += batch.items;
        }
        self.history.drain(..remove);
        self.history.push(input);
        let schema = Some(schema);
        let mut attempt = self.transport.start_batch();
        loop {
            let result = async {
                let parent = self.parent.upgrade().context("Guardian root stopped")?;
                let request = self.request(&schema);
                let pending = self
                    .transport
                    .stream(
                        &parent,
                        self.conversation,
                        &attempt,
                        request,
                        self.model.use_responses_lite,
                    )
                    .await?;
                drop(parent);
                Self::read_response(pending).await
            };
            match result.await {
                Ok((routes, output)) => {
                    return Ok((
                        routes,
                        SampledBatch {
                            input_tokens,
                            output,
                        },
                    ));
                }
                Err(error) => {
                    if !self
                        .transport
                        .retry_after_failure(&mut attempt, &error)
                        .await
                    {
                        return Err(error);
                    }
                }
            }
        }
    }

    fn request(&self, schema: &Option<Value>) -> ResponsesApiRequest {
        let mut input: Vec<_> = std::iter::once(&self.instructions)
            .chain(&self.history)
            .cloned()
            .collect();
        for item in &mut input {
            if !self.model.use_responses_lite {
                item.clear_content_item_metadata();
            }
            if !self.openai_provider {
                item.clear_internal_chat_message_metadata_passthrough();
                if !matches!(item, ResponseItem::Reasoning { .. }) {
                    item.clear_encrypted_content();
                }
            }
        }
        ResponsesApiRequest {
            request_country: Default::default(),
            model: self.model.slug.clone(),
            input,
            tools: None,
            tool_choice: "none".to_owned(),
            parallel_tool_calls: false,
            reasoning: Some(Reasoning {
                effort: Some(ReasoningEffort::Low),
                summary: None,
                context: self
                    .model
                    .use_responses_lite
                    .then_some(ReasoningContext::AllTurns),
            }),
            store: false,
            stream: true,
            stream_options: None,
            include: vec!["reasoning.encrypted_content".to_owned()],
            service_tier: self.service_tier.clone(),
            prompt_cache_key: Some(self.conversation.to_string()),
            prompt_cache_options: None,
            text: codex_api::create_text_param_for_request(
                /*verbosity*/ None, schema, /*output_schema_strict*/ true,
            ),
            client_metadata: None,
            access_programs: None,
        }
    }

    async fn read_response(
        mut pending: PendingResponse,
    ) -> anyhow::Result<(RoutingOutput, Vec<ResponseItem>)> {
        let mut output = Vec::new();
        while let Some(event) = pending.stream.rx_event.recv().await {
            match event? {
                ResponseEvent::OutputItemDone(item) => {
                    let retain = match &item {
                        ResponseItem::Reasoning { .. } => true,
                        ResponseItem::Message { role, content, .. } => {
                            role == "assistant"
                                && content
                                    .iter()
                                    .all(|part| matches!(part, ContentItem::OutputText { .. }))
                        }
                        _ => false,
                    };
                    ensure!(retain, "unexpected item in tool-free housekeeper response");
                    output.push(item);
                }
                ResponseEvent::Completed {
                    end_turn: Some(false),
                    ..
                } => anyhow::bail!("incomplete housekeeper turn"),
                ResponseEvent::Completed { .. } => {
                    drop(pending.stream.interrupt.take());
                    let text = output
                        .iter()
                        .rev()
                        .find_map(|item| match item {
                            ResponseItem::Message {
                                role,
                                phase: None | Some(MessagePhase::FinalAnswer),
                                content,
                                ..
                            } if role == "assistant" => codex_core::content_items_to_text(content),
                            _ => None,
                        })
                        .context("missing housekeeper routing reply")?;
                    let routes = serde_json::from_str(&text).map_err(|e| {
                        anyhow::anyhow!(
                            "housekeeper returned invalid JSON ({:?} at line {} column {})",
                            e.classify(),
                            e.line(),
                            e.column()
                        )
                    })?;
                    return Ok((routes, output));
                }
                _ => {}
            }
        }
        anyhow::bail!("housekeeper response ended before completion")
    }

    /// Called only after routing validation and board writes have succeeded.
    pub(super) fn commit(&mut self, batch: SampledBatch) {
        let output_tokens: usize = batch.output.iter().map(estimate_input_tokens).sum();
        let tokens = batch.input_tokens + output_tokens;
        self.batches.push_back(RetainedBatch {
            items: 1 + batch.output.len(),
            tokens,
        });
        self.history.extend(batch.output);
        self.retained_tokens += tokens;
    }
}
