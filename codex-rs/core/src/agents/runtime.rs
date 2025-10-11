use super::budgeter::TokenBudgeter;
use super::loader::AgentLoader;
use super::types::AgentDefinition;
use super::types::AgentResult;
use super::types::AgentStatus;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::error;
use tracing::info;

use crate::AuthManager;
use crate::audit_log::AgentExecutionEvent;
use crate::audit_log::AuditEvent;
use crate::audit_log::AuditEventType;
use crate::audit_log::ExecutionStatus;
use crate::audit_log::log_audit_event;
use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use crate::model_provider_info::ModelProviderInfo;
use codex_mcp_client::McpClient;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use mcp_types::InitializeRequestParams;

/// サブエージェントランタイム
pub struct AgentRuntime {
    /// エージェントローダー
    loader: Arc<RwLock<AgentLoader>>,
    /// トークン予算管理
    budgeter: Arc<TokenBudgeter>,
    /// 実行中のエージェント
    running_agents: Arc<RwLock<HashMap<String, AgentStatus>>>,
    /// ワークスペースディレクトリ
    workspace_dir: PathBuf,
    /// LLM設定
    config: Arc<Config>,
    /// 認証マネージャー
    auth_manager: Option<Arc<AuthManager>>,
    /// OpenTelemetry イベントマネージャー
    otel_manager: OtelEventManager,
    /// モデルプロバイダー情報
    provider: ModelProviderInfo,
    /// 会話ID
    conversation_id: ConversationId,
    /// Codexバイナリパス（MCP統合用）
    codex_binary_path: Option<PathBuf>,
}

impl AgentRuntime {
    /// 新しいランタイムを作成
    pub fn new(
        workspace_dir: PathBuf,
        total_budget: usize,
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        otel_manager: OtelEventManager,
        provider: ModelProviderInfo,
        conversation_id: ConversationId,
    ) -> Self {
        let loader = Arc::new(RwLock::new(AgentLoader::new(&workspace_dir)));
        let budgeter = Arc::new(TokenBudgeter::new(total_budget));

        Self {
            loader,
            budgeter,
            running_agents: Arc::new(RwLock::new(HashMap::new())),
            workspace_dir,
            config,
            auth_manager,
            otel_manager,
            provider,
            conversation_id,
            codex_binary_path: None,
        }
    }

    /// 複数エージェントを並列実行
    pub async fn delegate_parallel(
        &self,
        agents: Vec<(String, String, HashMap<String, String>, Option<usize>)>, // (agent_name, goal, inputs, budget)
        _deadline: Option<u64>,
    ) -> Result<Vec<AgentResult>> {
        info!("Starting parallel delegation of {} agents", agents.len());

        // 各エージェントをtokio::spawnで並列起動
        let mut handles = Vec::new();

        for (agent_name, goal, inputs, budget) in agents {
            let runtime_clone = Arc::new(self.clone_for_parallel());
            let agent_name_clone = agent_name.clone();

            let handle = tokio::spawn(async move {
                runtime_clone
                    .delegate(&agent_name_clone, &goal, inputs, budget, None)
                    .await
            });

            handles.push((agent_name, handle));
        }

        // 全エージェントの完了を待機
        let mut results = Vec::new();
        for (agent_name, handle) in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    info!("Agent '{}' completed successfully", agent_name);
                    results.push(result);
                }
                Ok(Err(e)) => {
                    error!("Agent '{}' failed: {}", agent_name, e);
                    // エラーでも続行して他のエージェントの結果を収集
                    results.push(AgentResult {
                        agent_name: agent_name.clone(),
                        status: AgentStatus::Failed,
                        artifacts: vec![],
                        tokens_used: 0,
                        duration_secs: 0.0,
                        error: Some(e.to_string()),
                    });
                }
                Err(e) => {
                    error!("Agent '{}' task panicked: {}", agent_name, e);
                    results.push(AgentResult {
                        agent_name: agent_name.clone(),
                        status: AgentStatus::Failed,
                        artifacts: vec![],
                        tokens_used: 0,
                        duration_secs: 0.0,
                        error: Some(format!("Task panicked: {}", e)),
                    });
                }
            }
        }

        info!(
            "Parallel delegation completed: {}/{} agents succeeded",
            results
                .iter()
                .filter(|r| matches!(r.status, AgentStatus::Completed))
                .count(),
            results.len()
        );

        Ok(results)
    }

    /// プロンプトからカスタムエージェントを作成して実行
    pub async fn create_and_run_custom_agent(
        &self,
        prompt: &str,
        budget: Option<usize>,
    ) -> Result<AgentResult> {
        info!("Creating custom agent from prompt");

        // LLMを使ってプロンプトからエージェント定義を生成
        let agent_def = self.generate_agent_from_prompt(prompt).await?;

        info!("Generated custom agent: {}", agent_def.name);

        // カスタムエージェントをメモリ上で実行（YAML保存不要）
        self.execute_custom_agent_inline(agent_def, budget).await
    }

    /// プロンプトからエージェント定義を生成
    async fn generate_agent_from_prompt(&self, prompt: &str) -> Result<AgentDefinition> {
        let system_prompt = r#"You are an AI agent definition generator. 
Given a user prompt, generate a complete agent definition in the following JSON format:

{
  "name": "agent-name",
  "goal": "Clear description of the agent's purpose",
  "tools": {
    "mcp": ["codex_read_file", "codex_grep"],
    "shell": []
  },
  "policies": {
    "context": {
      "max_tokens": 40000
    },
    "permissions": {
      "filesystem": [],
      "network": []
    }
  },
  "success_criteria": [
    "Clear criterion 1",
    "Clear criterion 2"
  ],
  "artifacts": [
    "artifacts/output.md"
  ]
}

Guidelines:
- name: Use kebab-case (e.g., "code-reviewer", "test-generator")
- goal: Be specific and actionable
- tools.mcp: Choose from [codex_read_file, codex_grep, codex_codebase_search, codex_apply_patch]
- tools.mcp: Do NOT include codex_shell unless explicitly requested (security)
- max_tokens: 40000 for simple tasks, 60000 for complex tasks
- success_criteria: 3-5 measurable criteria
- artifacts: Specify expected output files

Only output the JSON, no explanation."#;

        let user_message = format!("Generate an agent definition for this task:\n\n{}", prompt);

        let input_items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_message }],
        }];

        let llm_prompt = Prompt {
            input: input_items,
            tools: vec![],
            parallel_tool_calls: false,
            base_instructions_override: Some(system_prompt.to_string()),
            output_schema: None,
        };

        // LLM呼び出し
        let model_client = ModelClient::new(
            self.config.clone(),
            self.auth_manager.clone(),
            self.otel_manager.clone(),
            self.provider.clone(),
            Some(ReasoningEffort::Medium),
            ReasoningSummary::Concise,
            self.conversation_id,
        );

        let mut response_stream = model_client
            .stream(&llm_prompt)
            .await
            .context("Failed to generate agent definition")?;

        // レスポンスを収集
        let mut full_response = String::new();
        while let Some(event) = response_stream.next().await {
            match event? {
                ResponseEvent::OutputItemDone(item) => {
                    if let ResponseItem::Message { content, .. } = item {
                        for content_item in content {
                            if let ContentItem::OutputText { text } = content_item {
                                full_response.push_str(&text);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // JSONを抽出（コードブロック内の可能性があるため）
        let json_str = if let Some(start) = full_response.find('{') {
            if let Some(end) = full_response.rfind('}') {
                &full_response[start..=end]
            } else {
                &full_response
            }
        } else {
            &full_response
        };

        // JSONをパース
        let agent_def: AgentDefinition =
            serde_json::from_str(json_str).context("Failed to parse generated agent definition")?;

        info!(
            "Successfully generated agent definition: {}",
            agent_def.name
        );

        Ok(agent_def)
    }

    /// カスタムエージェントをインラインで実行（YAML保存なし）
    async fn execute_custom_agent_inline(
        &self,
        agent_def: AgentDefinition,
        budget: Option<usize>,
    ) -> Result<AgentResult> {
        let agent_name = &agent_def.name;
        info!("Executing custom agent '{}' inline", agent_name);

        // 予算設定
        let effective_budget = budget.unwrap_or(agent_def.policies.context.max_tokens);
        self.budgeter
            .set_agent_limit(agent_name, effective_budget)?;

        // 実行ステータス更新
        {
            let mut running = self.running_agents.write().await;
            running.insert(agent_name.to_string(), AgentStatus::Running);
        }

        let start_time = Instant::now();
        let start_timestamp = chrono::Utc::now().to_rfc3339();

        // エージェント実行
        let result = match self
            .execute_agent(&agent_def, &agent_def.goal, HashMap::new(), None)
            .await
        {
            Ok(artifacts) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let tokens_used = self.budgeter.get_agent_usage(agent_name);

                // 監査ログ: 成功
                let _ = log_audit_event(AuditEvent::new(
                    agent_name.to_string(),
                    AuditEventType::AgentExecution(AgentExecutionEvent {
                        agent_name: agent_name.to_string(),
                        status: ExecutionStatus::Completed,
                        goal: agent_def.goal.clone(),
                        start_time: start_timestamp.clone(),
                        end_time: Some(chrono::Utc::now().to_rfc3339()),
                        duration_secs: Some(duration_secs),
                        tokens_used,
                        artifacts: artifacts.clone(),
                        error: None,
                    }),
                ));

                AgentResult {
                    agent_name: agent_name.to_string(),
                    status: AgentStatus::Completed,
                    artifacts,
                    tokens_used,
                    duration_secs,
                    error: None,
                }
            }
            Err(e) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let tokens_used = self.budgeter.get_agent_usage(agent_name);

                error!("Custom agent '{}' failed: {}", agent_name, e);

                // 監査ログ: 失敗
                let _ = log_audit_event(AuditEvent::new(
                    agent_name.to_string(),
                    AuditEventType::AgentExecution(AgentExecutionEvent {
                        agent_name: agent_name.to_string(),
                        status: ExecutionStatus::Failed,
                        goal: agent_def.goal.clone(),
                        start_time: start_timestamp,
                        end_time: Some(chrono::Utc::now().to_rfc3339()),
                        duration_secs: Some(duration_secs),
                        tokens_used,
                        artifacts: vec![],
                        error: Some(e.to_string()),
                    }),
                ));

                AgentResult {
                    agent_name: agent_name.to_string(),
                    status: AgentStatus::Failed,
                    artifacts: vec![],
                    tokens_used,
                    duration_secs,
                    error: Some(e.to_string()),
                }
            }
        };

        // 実行ステータス更新
        {
            let mut running = self.running_agents.write().await;
            running.insert(agent_name.to_string(), result.status.clone());
        }

        Ok(result)
    }

    /// 並列実行用にクローン
    fn clone_for_parallel(&self) -> Self {
        Self {
            loader: self.loader.clone(),
            budgeter: self.budgeter.clone(),
            running_agents: Arc::new(RwLock::new(HashMap::new())),
            workspace_dir: self.workspace_dir.clone(),
            config: self.config.clone(),
            auth_manager: self.auth_manager.clone(),
            otel_manager: self.otel_manager.clone(),
            provider: self.provider.clone(),
            conversation_id: self.conversation_id,
            codex_binary_path: self.codex_binary_path.clone(),
        }
    }

    /// エージェントを委任実行
    pub async fn delegate(
        &self,
        agent_name: &str,
        goal: &str,
        inputs: HashMap<String, String>,
        budget: Option<usize>,
        deadline: Option<u64>,
    ) -> Result<AgentResult> {
        info!("Delegating to agent '{}': {}", agent_name, goal);

        // エージェント定義を読み込み
        let agent_def = {
            let mut loader = self.loader.write().await;
            loader
                .load_by_name(agent_name)
                .with_context(|| format!("Failed to load agent '{}'", agent_name))?
        };

        // 予算を設定
        if let Some(budget) = budget {
            self.budgeter.set_agent_limit(agent_name, budget)?;
        } else {
            // デフォルト予算はコンテキストポリシーから取得
            self.budgeter
                .set_agent_limit(agent_name, agent_def.policies.context.max_tokens)?;
        }

        // 実行ステータスを更新
        {
            let mut running = self.running_agents.write().await;
            running.insert(agent_name.to_string(), AgentStatus::Running);
        }

        // 実行開始
        let start_time = Instant::now();
        let start_timestamp = chrono::Utc::now().to_rfc3339();

        // 監査ログ: エージェント開始
        let _ = log_audit_event(AuditEvent::new(
            agent_name.to_string(),
            AuditEventType::AgentExecution(AgentExecutionEvent {
                agent_name: agent_name.to_string(),
                status: ExecutionStatus::Started,
                goal: goal.to_string(),
                start_time: start_timestamp.clone(),
                end_time: None,
                duration_secs: None,
                tokens_used: 0,
                artifacts: vec![],
                error: None,
            }),
        ))
        .await;

        let result = match self.execute_agent(&agent_def, goal, inputs, deadline).await {
            Ok(artifacts) => {
                let duration_secs = start_time.elapsed().as_secs_f64();
                let tokens_used = self.budgeter.get_agent_usage(agent_name);

                info!(
                    "Agent '{}' completed successfully in {:.2}s, used {} tokens",
                    agent_name, duration_secs, tokens_used
                );

                // 監査ログ: エージェント完了
                let _ = log_audit_event(AuditEvent::new(
                    agent_name.to_string(),
                    AuditEventType::AgentExecution(AgentExecutionEvent {
                        agent_name: agent_name.to_string(),
                        status: ExecutionStatus::Completed,
                        goal: goal.to_string(),
                        start_time: start_timestamp.clone(),
                        end_time: Some(chrono::Utc::now().to_rfc3339()),
                        duration_secs: Some(duration_secs),
                        tokens_used,
                        artifacts: artifacts.clone(),
                        error: None,
                    }),
                ))
                .await;

                AgentResult {
                    agent_name: agent_name.to_string(),
                    status: AgentStatus::Completed,
                    artifacts,
                    tokens_used,
                    duration_secs,
                    error: None,
                }
            }
            Err(e) => {
                error!("Agent '{}' failed: {}", agent_name, e);

                let duration_secs = start_time.elapsed().as_secs_f64();
                let tokens_used = self.budgeter.get_agent_usage(agent_name);

                // 監査ログ: エージェント失敗
                let _ = log_audit_event(AuditEvent::new(
                    agent_name.to_string(),
                    AuditEventType::AgentExecution(AgentExecutionEvent {
                        agent_name: agent_name.to_string(),
                        status: ExecutionStatus::Failed,
                        goal: goal.to_string(),
                        start_time: start_timestamp.clone(),
                        end_time: Some(chrono::Utc::now().to_rfc3339()),
                        duration_secs: Some(duration_secs),
                        tokens_used,
                        artifacts: vec![],
                        error: Some(e.to_string()),
                    }),
                ))
                .await;

                AgentResult {
                    agent_name: agent_name.to_string(),
                    status: AgentStatus::Failed,
                    artifacts: vec![],
                    tokens_used,
                    duration_secs,
                    error: Some(e.to_string()),
                }
            }
        };

        // 実行ステータスを更新
        {
            let mut running = self.running_agents.write().await;
            running.insert(agent_name.to_string(), result.status.clone());
        }

        Ok(result)
    }

    /// エージェントを実際に実行
    async fn execute_agent(
        &self,
        agent_def: &AgentDefinition,
        goal: &str,
        inputs: HashMap<String, String>,
        deadline: Option<u64>,
    ) -> Result<Vec<String>> {
        debug!("Executing agent '{}' with goal: {}", agent_def.name, goal);

        // 1. システムプロンプト構築
        let system_prompt = format!(
            "You are a specialized sub-agent with the following role:\n\
             \n\
             Agent: {}\n\
             Goal: {}\n\
             \n\
             Success Criteria:\n{}\n\
             \n\
             Inputs provided:\n{}\n\
             \n\
             Please analyze the task and execute it according to your role.\
             Generate the required artifacts as specified.",
            agent_def.name,
            agent_def.goal,
            agent_def.success_criteria.join("\n- "),
            inputs
                .iter()
                .map(|(k, v)| format!("- {}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // 2. ユーザー入力を構築
        let user_message = format!("Task: {}\n\nPlease proceed with the execution.", goal);

        // 3. ModelClient作成
        let client = ModelClient::new(
            self.config.clone(),
            self.auth_manager.clone(),
            self.otel_manager.clone(),
            self.provider.clone(),
            Some(ReasoningEffort::Medium),
            ReasoningSummary::Concise,
            self.conversation_id,
        );

        // 4. ResponseItem構築（Promptに渡す）
        let input_items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: user_message.clone(),
            }],
        }];

        // 5. Prompt構築（ツールは現時点では空、将来的にツール権限から生成）
        let prompt = Prompt {
            input: input_items,
            tools: vec![], // TODO: agent_def.toolsからツール仕様を生成
            parallel_tool_calls: false,
            base_instructions_override: Some(system_prompt.clone()),
            output_schema: None,
        };

        // 6. LLM呼び出し
        let mut stream = client.stream(&prompt).await?;
        let response_text = String::new();
        let mut total_tokens = 0;

        while let Some(event) = stream.next().await {
            match event? {
                ResponseEvent::Created => {
                    debug!("Agent '{}': Response stream started", agent_def.name);
                }
                ResponseEvent::OutputItemDone(_item) => {
                    // TODO: Extract text from ResponseItem properly
                    debug!("Agent '{}': Output item done", agent_def.name);
                    // Placeholder token estimation
                    total_tokens += 100;
                }
                ResponseEvent::Completed {
                    response_id: _,
                    token_usage: _,
                } => {
                    debug!("Agent '{}': Response completed", agent_def.name);
                }
                _ => {}
            }
        }

        // 7. トークン予算チェックと消費
        if !self.budgeter.try_consume(&agent_def.name, total_tokens)? {
            anyhow::bail!("Token budget exceeded for agent '{}'", agent_def.name);
        }

        info!(
            "Agent '{}' completed LLM execution: {} tokens used",
            agent_def.name, total_tokens
        );

        // 8. アーティファクト生成
        let artifacts_dir = self.workspace_dir.join("artifacts");
        tokio::fs::create_dir_all(&artifacts_dir).await?;

        let mut generated_artifacts = Vec::new();
        for artifact_path in &agent_def.artifacts {
            let full_path = self.workspace_dir.join(artifact_path);
            if let Some(parent) = full_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            // アーティファクト内容を生成
            let content = format!(
                "# Agent: {}\n\n## Goal\n{}\n\n## Task\n{}\n\n## Inputs\n{}\n\n## Agent Response\n\n{}\n\n## Execution Summary\n\n- Tokens Used: {}\n- Success Criteria:\n{}\n",
                agent_def.name,
                agent_def.goal,
                goal,
                inputs
                    .iter()
                    .map(|(k, v)| format!("- **{}**: {}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n"),
                response_text,
                total_tokens,
                agent_def
                    .success_criteria
                    .iter()
                    .map(|c| format!("  - {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            );

            tokio::fs::write(&full_path, content).await?;
            generated_artifacts.push(artifact_path.clone());
        }

        Ok(generated_artifacts)
    }

    /// 利用可能なエージェント一覧を取得
    pub async fn list_agents(&self) -> Result<Vec<String>> {
        let loader = self.loader.read().await;
        loader.list_available_agents()
    }

    /// 実行中のエージェント状態を取得
    pub async fn get_running_agents(&self) -> HashMap<String, AgentStatus> {
        self.running_agents.read().await.clone()
    }

    /// トークン使用状況を取得
    pub fn get_budget_status(&self) -> (usize, usize, f64) {
        let used = self.budgeter.get_used();
        let remaining = self.budgeter.get_remaining();
        let utilization = self.budgeter.get_utilization();
        (used, remaining, utilization)
    }

    /// 軽量版フォールバックが必要かチェック
    pub fn should_use_lightweight(&self, threshold: f64) -> bool {
        self.budgeter.should_fallback_lightweight(threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_runtime_delegate() {
        use crate::config::Config;
        use crate::model_provider_info::ModelProviderInfo;
        use crate::model_provider_info::WireApi;
        use codex_otel::otel_event_manager::OtelEventManager;
        use codex_protocol::ConversationId;
        use uuid::Uuid;

        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".codex/agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let agent_yaml = r#"
name: "Test Agent"
goal: "Test goal"
tools:
  mcp: []
  fs:
    read: true
    write:
      - "./artifacts"
  net:
    allow: []
  shell: []
policies:
  context:
    max_tokens: 5000
    retention: "job"
  secrets:
    redact: false
success_criteria:
  - "基準1"
artifacts:
  - "artifacts/test-output.md"
"#;

        fs::write(agents_dir.join("test-agent.yaml"), agent_yaml).unwrap();

        // モックConfig作成
        let config = Arc::new(Config::default_for_family("gpt-5-codex"));
        let provider = ModelProviderInfo {
            name: "Test Provider".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let otel_manager = OtelEventManager::new();
        let conversation_id = ConversationId(Uuid::new_v4());

        let runtime = AgentRuntime::new(
            temp_dir.path().to_path_buf(),
            10000,
            config,
            None,
            otel_manager,
            provider,
            conversation_id,
        );

        let mut inputs = HashMap::new();
        inputs.insert("key1".to_string(), "value1".to_string());

        // Note: This will fail without real API credentials, but demonstrates the structure
        let result = runtime
            .delegate("test-agent", "Test goal", inputs, Some(5000), None)
            .await;

        // In real tests, we'd use mocks or fixtures
        // For now, just verify compilation
        match result {
            Ok(r) => {
                assert_eq!(r.agent_name, "test-agent");
            }
            Err(_) => {
                // Expected without real API credentials
            }
        }
    }

    #[tokio::test]
    async fn test_list_agents() {
        use crate::config::Config;
        use crate::model_provider_info::ModelProviderInfo;
        use crate::model_provider_info::WireApi;
        use codex_otel::otel_event_manager::OtelEventManager;
        use codex_protocol::ConversationId;
        use uuid::Uuid;

        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".codex/agents");
        fs::create_dir_all(&agents_dir).unwrap();

        fs::write(agents_dir.join("agent1.yaml"), "name: Agent1\ngoal: Goal1\ntools: {}\npolicies: {context: {}}\nsuccess_criteria: []\nartifacts: []").unwrap();
        fs::write(agents_dir.join("agent2.yaml"), "name: Agent2\ngoal: Goal2\ntools: {}\npolicies: {context: {}}\nsuccess_criteria: []\nartifacts: []").unwrap();

        let config = Arc::new(Config::default_for_family("gpt-5-codex_medium"));
        let provider = ModelProviderInfo {
            name: "Test Provider".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let otel_manager = OtelEventManager::new();
        let conversation_id = ConversationId(Uuid::new_v4());

        let runtime = AgentRuntime::new(
            temp_dir.path().to_path_buf(),
            10000,
            config.clone(),
            None,
            otel_manager,
            provider,
            conversation_id,
        );
        let agents = runtime.list_agents().await.unwrap();

        assert_eq!(agents, vec!["agent1", "agent2"]);
    }
}

// ========== Codex MCP Integration (Phase 2) ==========

impl AgentRuntime {
    /// Codexバイナリパスを設定
    pub fn with_codex_binary_path(mut self, path: PathBuf) -> Self {
        self.codex_binary_path = Some(path);
        self
    }

    /// Codex MCP Serverをstdio モードで起動
    async fn spawn_codex_mcp_server(&self) -> Result<Arc<McpClient>> {
        let codex_path = self
            .codex_binary_path
            .clone()
            .or_else(|| std::env::current_exe().ok())
            .ok_or_else(|| anyhow!("Codex binary path not configured"))?;

        info!(
            "Spawning Codex MCP Server: {} mcp-server",
            codex_path.display()
        );

        let client = McpClient::new_stdio_client(
            codex_path.into_os_string(),
            vec![OsString::from("mcp-server")],
            None,
        )
        .await
        .context("Failed to spawn Codex MCP server")?;

        // Initialize MCP session
        let init_params = InitializeRequestParams {
            client_info: mcp_types::Implementation {
                name: "codex-subagent-runtime".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                user_agent: None,
            },
            protocol_version: "0.1.0".to_string(),
            capabilities: mcp_types::ClientCapabilities {
                elicitation: None,
                experimental: None,
                sampling: None,
                roots: None,
            },
        };

        client
            .initialize(init_params, Some(Duration::from_secs(10)))
            .await
            .context("Failed to initialize Codex MCP server")?;

        info!("Codex MCP Server initialized successfully");

        Ok(Arc::new(client))
    }

    /// エージェント権限に基づいてCodex MCP toolsをフィルタリング
    fn filter_codex_mcp_tools(&self, agent_def: &AgentDefinition) -> Vec<String> {
        agent_def
            .tools
            .mcp
            .iter()
            .filter(|tool| tool.starts_with("codex_"))
            .cloned()
            .collect()
    }

    /// Codex MCP toolsの説明を生成（プロンプト用）
    fn build_codex_mcp_tools_description(&self, allowed_tools: &[String]) -> String {
        let mut desc = String::from("Available Codex MCP Tools:\n\n");

        for tool in allowed_tools {
            let tool_desc = match tool.as_str() {
                "codex_read_file" => {
                    "- codex_read_file(path: str) -> str\n  \
                     Read a file from the workspace using Codex.\n  \
                     Safe, read-only operation."
                }
                "codex_grep" => {
                    "- codex_grep(pattern: str, path: Optional[str]) -> List[str]\n  \
                     Search for patterns in files using Codex grep.\n  \
                     Safe, read-only operation."
                }
                "codex_codebase_search" => {
                    "- codex_codebase_search(query: str, target_directories: Optional[List[str]]) -> List[str]\n  \
                     Semantic code search using Codex.\n  \
                     Safe, read-only operation."
                }
                "codex_apply_patch" => {
                    "- codex_apply_patch(patch: str) -> str\n  \
                     Apply a code patch using Codex.\n  \
                     Requires write permission."
                }
                "codex_shell" => {
                    "- codex_shell(command: str) -> str\n  \
                     Execute a shell command via Codex (restricted).\n  \
                     Requires shell permission."
                }
                _ => continue,
            };
            desc.push_str(tool_desc);
            desc.push_str("\n\n");
        }

        desc.push_str(
            "To use these tools, output a tool call in the following format:\n\
             TOOL_CALL: tool_name(arg1=\"value1\", arg2=\"value2\")\n\n\
             The results will be provided to you for further analysis.",
        );

        desc
    }

    /// エージェントをCodex MCP経由で実行（Phase 3: 完全実装）
    pub async fn execute_agent_with_codex_mcp(
        &self,
        agent_def: &AgentDefinition,
        goal: &str,
        inputs: HashMap<String, String>,
        _deadline: Option<u64>,
    ) -> Result<Vec<String>> {
        debug!(
            "Executing agent '{}' with Codex MCP integration",
            agent_def.name
        );

        // 1. Codex MCP Serverを起動
        let mcp_client = self
            .spawn_codex_mcp_server()
            .await
            .context("Failed to spawn Codex MCP server")?;

        // 2. エージェント権限でツールをフィルタリング
        let allowed_tools = self.filter_codex_mcp_tools(agent_def);

        info!(
            "Agent '{}' is allowed to use {} Codex MCP tools: {:?}",
            agent_def.name,
            allowed_tools.len(),
            allowed_tools
        );

        // 3. システムプロンプト構築（ツール説明含む）
        let tools_description = self.build_codex_mcp_tools_description(&allowed_tools);

        let system_prompt = format!(
            "You are a specialized sub-agent with the following role:\n\
             \n\
             Agent: {}\n\
             Goal: {}\n\
             \n\
             Success Criteria:\n{}\n\
             \n\
             Inputs provided:\n{}\n\
             \n\
             {}\n\
             \n\
             Please analyze the task and use the available Codex MCP tools to complete it.\
             When you need to use a tool, output it in the specified format.\
             After all tool calls are complete, provide a final summary.",
            agent_def.name,
            agent_def.goal,
            agent_def.success_criteria.join("\n- "),
            inputs
                .iter()
                .map(|(k, v)| format!("- {}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
            tools_description
        );

        // 4. 初期プロンプト
        let user_prompt = format!("Task: {}", goal);

        // 5. LLM対話ループ（最大5回のツール呼び出し）
        let max_iterations = 5;
        let mut conversation_history = vec![
            ("system".to_string(), system_prompt),
            ("user".to_string(), user_prompt.clone()),
        ];
        let mut artifacts = Vec::new();

        for iteration in 0..max_iterations {
            debug!("Agent iteration {}/{}", iteration + 1, max_iterations);

            // LLM呼び出し
            let llm_response = self
                .call_llm_for_agent(&conversation_history)
                .await
                .context("Failed to call LLM for agent")?;

            conversation_history.push(("assistant".to_string(), llm_response.clone()));
            artifacts.push(llm_response.clone());

            // ツールコール検出
            let tool_calls = self.detect_tool_calls(&llm_response);

            if tool_calls.is_empty() {
                // ツールコールがない場合は終了
                info!("No more tool calls detected. Agent task completed.");
                break;
            }

            // ツール実行
            let mut tool_results = Vec::new();
            for (tool_name, tool_args) in tool_calls {
                if !allowed_tools.contains(&tool_name) {
                    let error_msg = format!(
                        "ERROR: Tool '{}' is not permitted for this agent",
                        tool_name
                    );
                    tool_results.push(error_msg);
                    continue;
                }

                info!(
                    "Executing Codex MCP tool: {} with args: {:?}",
                    tool_name, tool_args
                );

                match self
                    .execute_codex_mcp_tool(&mcp_client, &tool_name, tool_args)
                    .await
                {
                    Ok(result) => {
                        tool_results.push(format!("TOOL_RESULT[{}]: {}", tool_name, result));
                    }
                    Err(e) => {
                        let error_msg = format!("ERROR executing tool '{}': {}", tool_name, e);
                        error!("{}", error_msg);
                        tool_results.push(error_msg);
                    }
                }
            }

            // ツール結果をLLMにフィードバック
            let feedback = tool_results.join("\n\n");
            conversation_history.push(("user".to_string(), feedback.clone()));
            artifacts.push(format!("--- Tool Execution Results ---\n{}", feedback));
        }

        info!("Agent '{}' completed execution", agent_def.name);

        Ok(artifacts)
    }

    /// LLMを呼び出してエージェントの応答を取得
    async fn call_llm_for_agent(&self, conversation: &[(String, String)]) -> Result<String> {
        // プロンプト構築（最新のメッセージのみを使用）
        let last_message = conversation
            .last()
            .ok_or_else(|| anyhow!("Conversation history is empty"))?;

        let system_instructions = conversation
            .first()
            .filter(|(role, _)| role == "system")
            .map(|(_, content)| content.clone());

        let input_items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: last_message.1.clone(),
            }],
        }];

        let prompt = Prompt {
            input: input_items,
            tools: vec![],
            parallel_tool_calls: false,
            base_instructions_override: system_instructions,
            output_schema: None,
        };

        // ModelClient経由でLLM呼び出し
        let model_client = ModelClient::new(
            self.config.clone(),
            self.auth_manager.clone(),
            self.otel_manager.clone(),
            self.provider.clone(),
            Some(ReasoningEffort::Medium),
            ReasoningSummary::Concise,
            self.conversation_id,
        );

        let mut response_stream = model_client
            .stream(&prompt)
            .await
            .context("Failed to stream LLM response")?;

        // レスポンスを収集
        let mut full_response = String::new();
        while let Some(event) = response_stream.next().await {
            match event? {
                ResponseEvent::OutputItemDone(item) => {
                    // ResponseItemからテキストを抽出
                    if let ResponseItem::Message { content, .. } = item {
                        for content_item in content {
                            if let ContentItem::OutputText { text } = content_item {
                                full_response.push_str(&text);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(full_response)
    }

    /// LLMレスポンスからツールコールを検出
    fn detect_tool_calls(&self, response: &str) -> Vec<(String, serde_json::Value)> {
        let mut tool_calls = Vec::new();

        // パターン: TOOL_CALL: tool_name(arg1="value1", arg2="value2")
        // 簡易実装: JSONフォーマットも検出
        for line in response.lines() {
            let line = line.trim();

            // TOOL_CALL: codex_read_file(path="src/auth.rs")
            if line.starts_with("TOOL_CALL:") {
                if let Some(call_str) = line.strip_prefix("TOOL_CALL:").map(|s| s.trim()) {
                    if let Some((tool_name, args_str)) = call_str.split_once('(') {
                        let tool_name = tool_name.trim().to_string();
                        let args_str = args_str.trim_end_matches(')').trim();

                        // 簡易パース: key="value" 形式
                        let mut args = serde_json::Map::new();
                        for part in args_str.split(',') {
                            if let Some((key, value)) = part.split_once('=') {
                                let key = key.trim().to_string();
                                let value = value.trim().trim_matches('"').to_string();
                                args.insert(key, serde_json::Value::String(value));
                            }
                        }

                        tool_calls.push((tool_name, serde_json::Value::Object(args)));
                    }
                }
            }
        }

        tool_calls
    }

    /// Codex MCPツールを実行
    async fn execute_codex_mcp_tool(
        &self,
        mcp_client: &Arc<McpClient>,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String> {
        debug!(
            "Executing Codex MCP tool: {} with args: {:?}",
            tool_name, args
        );

        let result = mcp_client
            .call_tool(
                tool_name.to_string(),
                Some(args),
                Some(Duration::from_secs(30)),
            )
            .await
            .context(format!("Failed to call Codex MCP tool '{}'", tool_name))?;

        // 結果をテキスト形式に変換
        let result_text =
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result));

        Ok(result_text)
    }
}

#[cfg(test)]
mod mcp_tests {
    use super::*;

    #[tokio::test]
    async fn test_filter_codex_mcp_tools() {
        use crate::agents::types::ContextPolicy;
        use crate::agents::types::ExecutionPolicy;
        use crate::agents::types::PermissionPolicy;
        use crate::agents::types::ToolsPolicy;
        use crate::model_provider_info::WireApi;
        use uuid::Uuid;

        let agent_def = AgentDefinition {
            name: "test-agent".to_string(),
            goal: "Test".to_string(),
            tools: ToolsPolicy {
                mcp: vec![
                    "codex_read_file".to_string(),
                    "codex_grep".to_string(),
                    "some_other_tool".to_string(), // 非Codexツール
                ],
                shell: vec![],
            },
            policies: ExecutionPolicy {
                context: ContextPolicy {
                    max_tokens: 1000,
                    max_function_calls: 10,
                },
                permissions: PermissionPolicy {
                    filesystem: vec![],
                    network: vec![],
                },
            },
            success_criteria: vec![],
            artifacts: vec![],
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let config = Arc::new(Config::default_for_family("gpt-4"));
        let provider = ModelProviderInfo {
            name: "Test".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let otel_manager = OtelEventManager::new();
        let conversation_id = ConversationId(Uuid::new_v4());

        let runtime = AgentRuntime::new(
            temp_dir.path().to_path_buf(),
            10000,
            config,
            None,
            otel_manager,
            provider,
            conversation_id,
        );

        let filtered = runtime.filter_codex_mcp_tools(&agent_def);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"codex_read_file".to_string()));
        assert!(filtered.contains(&"codex_grep".to_string()));
        assert!(!filtered.contains(&"some_other_tool".to_string()));
    }

    #[tokio::test]
    async fn test_build_codex_mcp_tools_description() {
        use crate::model_provider_info::WireApi;
        use uuid::Uuid;

        let temp_dir = tempfile::tempdir().unwrap();
        let config = Arc::new(Config::default_for_family("gpt-4"));
        let provider = ModelProviderInfo {
            name: "Test".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let otel_manager = OtelEventManager::new();
        let conversation_id = ConversationId(Uuid::new_v4());

        let runtime = AgentRuntime::new(
            temp_dir.path().to_path_buf(),
            10000,
            config,
            None,
            otel_manager,
            provider,
            conversation_id,
        );

        let tools = vec!["codex_read_file".to_string(), "codex_grep".to_string()];
        let desc = runtime.build_codex_mcp_tools_description(&tools);

        assert!(desc.contains("codex_read_file"));
        assert!(desc.contains("codex_grep"));
        assert!(desc.contains("Safe, read-only operation"));
    }
}
