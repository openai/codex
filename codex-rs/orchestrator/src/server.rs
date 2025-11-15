/// Orchestrator RPC server
///
/// Single-Writer Queue architecture with idempotency cache
use crate::auth::AuthManager;
use crate::rpc::*;
use crate::transport::Connection;
use crate::transport::Transport;
use crate::transport::TransportConfig;
use crate::transport::TransportInfo;
use anyhow::Context;
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Idempotency cache entry
#[derive(Debug, Clone)]
struct IdempotencyEntry {
    response: RpcResponse,
    expires_at: SystemTime,
}

/// Orchestrator server state
pub struct OrchestratorServer {
    /// Server configuration
    config: OrchestratorConfig,
    /// Transport layer
    transport: Box<dyn Transport>,
    /// Authentication manager
    auth_manager: Arc<AuthManager>,
    /// Idempotency cache (idem_key -> response)
    idempotency_cache: Arc<RwLock<HashMap<String, IdempotencyEntry>>>,
    /// Single-writer queue
    write_queue: mpsc::Sender<WriteRequest>,
    /// Write queue receiver (for processing)
    write_queue_rx: Option<mpsc::Receiver<WriteRequest>>,
    /// Server start time
    start_time: SystemTime,
    /// Active agents
    active_agents: Arc<RwLock<HashMap<String, AgentInfo>>>,
    /// Active tasks
    active_tasks: Arc<RwLock<HashMap<String, TaskInfo>>>,
    /// Token budget tracker
    token_budget: Arc<RwLock<TokenBudget>>,
    /// PubSub subscribers
    subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>, // topic -> connection_ids
}

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub queue_capacity: usize,
    pub transport_config: TransportConfig,
    pub codex_dir: PathBuf,
    pub total_token_budget: u64,
    pub warning_threshold: u64,
    pub per_agent_limit: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 1024,
            transport_config: TransportConfig::default(),
            codex_dir: dirs::home_dir().unwrap_or_default().join(".codex"),
            total_token_budget: 100_000,
            warning_threshold: 80_000,
            per_agent_limit: 20_000,
        }
    }
}

/// Write request (queued for single-writer processing)
#[derive(Debug)]
struct WriteRequest {
    request: RpcRequest,
    response_tx: tokio::sync::oneshot::Sender<RpcResponse>,
}

/// Task information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TaskInfo {
    task_id: String,
    agent_type: String,
    status: String,
    submitted_at: SystemTime,
}

/// Token budget tracker
#[derive(Debug, Clone)]
struct TokenBudget {
    total_budget: u64,
    used: u64,
    warning_threshold: u64,
    per_agent_usage: HashMap<String, u64>,
}

impl OrchestratorServer {
    /// Create a new orchestrator server
    pub async fn new(config: OrchestratorConfig) -> Result<Self> {
        // Create transport
        let transport =
            crate::transport::create_transport(config.transport_config.clone(), &config.codex_dir)
                .await
                .context("Failed to create transport")?;

        // Load or create authentication manager
        let auth_manager = Arc::new(
            AuthManager::new(&config.codex_dir).context("Failed to initialize auth manager")?,
        );

        // Create single-writer queue
        let (write_queue_tx, write_queue_rx) = mpsc::channel::<WriteRequest>(config.queue_capacity);

        let token_budget = Arc::new(RwLock::new(TokenBudget {
            total_budget: config.total_token_budget,
            used: 0,
            warning_threshold: config.warning_threshold,
            per_agent_usage: HashMap::new(),
        }));

        Ok(Self {
            config,
            transport,
            auth_manager,
            idempotency_cache: Arc::new(RwLock::new(HashMap::new())),
            write_queue: write_queue_tx,
            write_queue_rx: Some(write_queue_rx),
            start_time: SystemTime::now(),
            active_agents: Arc::new(RwLock::new(HashMap::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            token_budget,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get transport info
    pub fn transport_info(&self) -> TransportInfo {
        self.transport.info()
    }

    /// Start the orchestrator server
    pub async fn run(&mut self) -> Result<()> {
        // Take ownership of write_queue_rx
        let mut write_queue_rx = self
            .write_queue_rx
            .take()
            .context("Server already running")?;

        // Spawn write queue processor
        let auth_manager = Arc::clone(&self.auth_manager);
        let active_agents = Arc::clone(&self.active_agents);
        let active_tasks = Arc::clone(&self.active_tasks);
        let token_budget = Arc::clone(&self.token_budget);
        let subscribers = Arc::clone(&self.subscribers);
        let config = self.config.clone();

        tokio::spawn(async move {
            while let Some(write_req) = write_queue_rx.recv().await {
                let response = Self::process_write_request(
                    &write_req.request,
                    &config,
                    &auth_manager,
                    &active_agents,
                    &active_tasks,
                    &token_budget,
                    &subscribers,
                )
                .await;

                let _ = write_req.response_tx.send(response);
            }
        });

        // Spawn idempotency cache cleanup task
        let idempotency_cache = Arc::clone(&self.idempotency_cache);
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(60)).await;
                Self::cleanup_idempotency_cache(&idempotency_cache).await;
            }
        });

        // Accept connections
        loop {
            match self.transport.accept().await {
                Ok(mut conn) => {
                    let auth_manager = Arc::clone(&self.auth_manager);
                    let idempotency_cache = Arc::clone(&self.idempotency_cache);
                    let write_queue = self.write_queue.clone();
                    let start_time = self.start_time;
                    let active_agents = Arc::clone(&self.active_agents);
                    let active_tasks = Arc::clone(&self.active_tasks);
                    let token_budget = Arc::clone(&self.token_budget);
                    let config = self.config.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(
                            conn.as_mut(),
                            &auth_manager,
                            &idempotency_cache,
                            &write_queue,
                            start_time,
                            &active_agents,
                            &active_tasks,
                            &token_budget,
                            &config,
                        )
                        .await
                        {
                            eprintln!("Connection error: {e}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {e}");
                }
            }
        }
    }

    /// Handle a client connection
    async fn handle_connection(
        conn: &mut dyn Connection,
        _auth_manager: &Arc<AuthManager>,
        idempotency_cache: &Arc<RwLock<HashMap<String, IdempotencyEntry>>>,
        write_queue: &mpsc::Sender<WriteRequest>,
        start_time: SystemTime,
        active_agents: &Arc<RwLock<HashMap<String, AgentInfo>>>,
        active_tasks: &Arc<RwLock<HashMap<String, TaskInfo>>>,
        token_budget: &Arc<RwLock<TokenBudget>>,
        config: &OrchestratorConfig,
    ) -> Result<()> {
        loop {
            // Read request
            let data = conn.read_message().await?;

            // Parse request
            let request: RpcRequest = match serde_json::from_slice(&data) {
                Ok(req) => req,
                Err(e) => {
                    let error_response = RpcResponse {
                        id: "".to_string(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_PARSE,
                            message: format!("Parse error: {e}"),
                            data: None,
                        }),
                    };
                    let response_data = serde_json::to_vec(&error_response)?;
                    conn.write_message(&response_data).await?;
                    continue;
                }
            };

            // Check idempotency cache
            if let Some(idem_key) = &request.idem_key {
                let cache = idempotency_cache.read().await;
                if let Some(entry) = cache.get(idem_key)
                    && entry.expires_at > SystemTime::now()
                {
                    // Return cached response
                    let response_data = serde_json::to_vec(&entry.response)?;
                    conn.write_message(&response_data).await?;
                    continue;
                }
            }

            // Process request
            let response = if Self::is_write_method(&request.method) {
                // Queue write request
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                let write_req = WriteRequest {
                    request: request.clone(),
                    response_tx,
                };

                match write_queue.try_send(write_req) {
                    Ok(_) => {
                        // Wait for response
                        match response_rx.await {
                            Ok(resp) => resp,
                            Err(_) => RpcResponse {
                                id: request.id.clone(),
                                result: None,
                                error: Some(RpcError {
                                    code: ERROR_INTERNAL,
                                    message: "Write queue processing failed".to_string(),
                                    data: None,
                                }),
                            },
                        }
                    }
                    Err(_) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_BACKPRESSURE,
                            message: "Write queue full".to_string(),
                            data: None,
                        }),
                    },
                }
            } else {
                // Handle read-only request directly
                Self::process_read_request(
                    &request,
                    start_time,
                    active_agents,
                    active_tasks,
                    token_budget,
                    config,
                )
                .await
            };

            // Cache response if idempotency key provided
            if let Some(idem_key) = &request.idem_key {
                let mut cache = idempotency_cache.write().await;
                cache.insert(
                    idem_key.clone(),
                    IdempotencyEntry {
                        response: response.clone(),
                        expires_at: SystemTime::now() + Duration::from_secs(600), // 10 min TTL
                    },
                );
            }

            // Send response
            let response_data = serde_json::to_vec(&response)?;
            conn.write_message(&response_data).await?;
        }
    }

    /// Check if method is a write operation
    fn is_write_method(method: &str) -> bool {
        matches!(
            method,
            "lock.acquire"
                | "lock.release"
                | "fs.write"
                | "fs.patch"
                | "vcs.commit"
                | "vcs.push"
                | "agent.register"
                | "task.submit"
                | "task.cancel"
                | "tokens.reportUsage"
                | "session.start"
                | "session.end"
                | "blueprint.create"
                | "blueprint.update"
                | "blueprint.approve"
                | "blueprint.reject"
                | "blueprint.export"
                | "blueprint.setMode"
                | "blueprint.addResearch"
        )
    }

    /// Process read-only request
    async fn process_read_request(
        request: &RpcRequest,
        start_time: SystemTime,
        active_agents: &Arc<RwLock<HashMap<String, AgentInfo>>>,
        active_tasks: &Arc<RwLock<HashMap<String, TaskInfo>>>,
        token_budget: &Arc<RwLock<TokenBudget>>,
        config: &OrchestratorConfig,
    ) -> RpcResponse {
        match request.method.as_str() {
            "status.get" => {
                let agents = active_agents.read().await;
                let tasks = active_tasks.read().await;
                let budget = token_budget.read().await;
                let uptime = SystemTime::now()
                    .duration_since(start_time)
                    .unwrap_or_default()
                    .as_secs();

                RpcResponse {
                    id: request.id.clone(),
                    result: Some(json!({
                        "server_version": env!("CARGO_PKG_VERSION"),
                        "uptime_seconds": uptime,
                        "queue_size": 0, // TODO: track actual queue size
                        "queue_capacity": config.queue_capacity,
                        "active_agents": agents.len(),
                        "active_tasks": tasks.len(),
                        "total_tokens_used": budget.used,
                        "total_tokens_budget": budget.total_budget,
                    })),
                    error: None,
                }
            }
            "lock.status" => {
                // TODO: Implement lock status check
                RpcResponse {
                    id: request.id.clone(),
                    result: Some(json!({
                        "locked": false,
                    })),
                    error: None,
                }
            }
            "agent.list" => {
                let agents = active_agents.read().await;
                let agent_list: Vec<_> = agents.values().cloned().collect();

                RpcResponse {
                    id: request.id.clone(),
                    result: Some(
                        serde_json::to_value(AgentListResponse { agents: agent_list })
                            .unwrap_or_else(|e| {
                                serde_json::json!({
                                    "error": format!("Serialization failed: {e}")
                                })
                            }),
                    ),
                    error: None,
                }
            }
            "tokens.getBudget" => {
                let budget = token_budget.read().await;
                RpcResponse {
                    id: request.id.clone(),
                    result: Some(json!({
                        "total_budget": budget.total_budget,
                        "used": budget.used,
                        "remaining": budget.total_budget.saturating_sub(budget.used),
                        "warning_threshold": budget.warning_threshold,
                    })),
                    error: None,
                }
            }
            "blueprint.get" => {
                let params: Result<BlueprintGetRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "blueprint": {
                                    "id": "example-bp",
                                    "title": "Example Blueprint",
                                    "goal": "Demonstrate blueprint structure",
                                    "state": "drafting",
                                }
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            _ => RpcResponse {
                id: request.id.clone(),
                result: None,
                error: Some(RpcError {
                    code: ERROR_METHOD_NOT_FOUND,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            },
        }
    }

    /// Process write request (in single-writer queue)
    async fn process_write_request(
        request: &RpcRequest,
        config: &OrchestratorConfig,
        _auth_manager: &Arc<AuthManager>,
        active_agents: &Arc<RwLock<HashMap<String, AgentInfo>>>,
        active_tasks: &Arc<RwLock<HashMap<String, TaskInfo>>>,
        token_budget: &Arc<RwLock<TokenBudget>>,
        _subscribers: &Arc<RwLock<HashMap<String, Vec<String>>>>,
    ) -> RpcResponse {
        match request.method.as_str() {
            "agent.register" => {
                // Parse params
                let params: Result<AgentRegisterRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(params) => {
                        let mut agents = active_agents.write().await;
                        agents.insert(
                            params.agent_id.clone(),
                            AgentInfo {
                                agent_id: params.agent_id.clone(),
                                agent_type: params.agent_type.clone(),
                                status: "active".to_string(),
                                last_heartbeat: chrono::Utc::now().to_rfc3339(),
                            },
                        );

                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({ "success": true })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "task.submit" => {
                let params: Result<TaskSubmitRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(params) => {
                        let mut tasks = active_tasks.write().await;
                        tasks.insert(
                            params.task_id.clone(),
                            TaskInfo {
                                task_id: params.task_id.clone(),
                                agent_type: params.agent_type.clone(),
                                status: "pending".to_string(),
                                submitted_at: SystemTime::now(),
                            },
                        );

                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "success": true,
                                "task_id": params.task_id,
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "tokens.reportUsage" => {
                let params: Result<TokensReportUsageRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(params) => {
                        let mut budget = token_budget.write().await;
                        budget.used += params.tokens_used;
                        *budget.per_agent_usage.entry(params.agent_id).or_insert(0) +=
                            params.tokens_used;

                        let remaining = budget.total_budget.saturating_sub(budget.used);

                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "success": true,
                                "remaining_budget": remaining,
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            // Blueprint methods (stubbed for now - will be fully implemented)
            "blueprint.create" => {
                let params: Result<BlueprintCreateRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "success": true,
                                "blueprint_id": format!("bp-{}", chrono::Utc::now().timestamp()),
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.update" => {
                let params: Result<BlueprintUpdateRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "success": true,
                                "blueprint_id": params.blueprint_id,
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.approve" => {
                let params: Result<BlueprintApproveRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({ "success": true })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.reject" => {
                let params: Result<BlueprintRejectRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({ "success": true })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.export" => {
                let params: Result<BlueprintExportRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({
                                "success": true,
                                "markdown_path": "docs/blueprints/example.md",
                                "json_path": "logs/blueprint/example.json",
                            })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.setMode" => {
                let params: Result<BlueprintSetModeRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement global mode setting
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({ "success": true })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            "blueprint.addResearch" => {
                let params: Result<BlueprintAddResearchRequest, _> =
                    serde_json::from_value(request.params.clone());
                match params {
                    Ok(_params) => {
                        // TODO: Implement with BlueprintManager
                        RpcResponse {
                            id: request.id.clone(),
                            result: Some(json!({ "success": true })),
                            error: None,
                        }
                    }
                    Err(e) => RpcResponse {
                        id: request.id.clone(),
                        result: None,
                        error: Some(RpcError {
                            code: ERROR_INVALID_PARAMS,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            _ => RpcResponse {
                id: request.id.clone(),
                result: None,
                error: Some(RpcError {
                    code: ERROR_METHOD_NOT_FOUND,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            },
        }
    }

    /// Cleanup expired idempotency cache entries
    async fn cleanup_idempotency_cache(cache: &Arc<RwLock<HashMap<String, IdempotencyEntry>>>) {
        let mut cache = cache.write().await;
        let now = SystemTime::now();
        cache.retain(|_, entry| entry.expires_at > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[test]
    fn test_is_write_method() {
        assert!(OrchestratorServer::is_write_method("lock.acquire"));
        assert!(OrchestratorServer::is_write_method("fs.write"));
        assert!(!OrchestratorServer::is_write_method("status.get"));
        assert!(!OrchestratorServer::is_write_method("lock.status"));
    }

    #[tokio::test]
    async fn test_orchestrator_config_defaults() {
        let config = OrchestratorConfig {
            queue_capacity: 1024,
            transport_config: TransportConfig::default(),
            codex_dir: dirs::home_dir().unwrap_or_default().join(".codex"),
            total_token_budget: 100_000,
            warning_threshold: 80_000,
            per_agent_limit: 20_000,
        };

        assert_eq!(config.queue_capacity, 1024);
        assert_eq!(config.total_token_budget, 100_000);
        assert_eq!(config.warning_threshold, 80_000);
        assert_eq!(config.per_agent_limit, 20_000);
    }

    #[tokio::test]
    async fn test_token_budget_initialization() {
        let budget = TokenBudget {
            total_budget: 1_000_000,
            used: 0,
            warning_threshold: 800_000,
            per_agent_usage: HashMap::new(),
        };

        assert_eq!(budget.total_budget, 1_000_000);
        assert_eq!(budget.used, 0);
        assert_eq!(budget.warning_threshold, 800_000);
        assert!(budget.per_agent_usage.is_empty());
    }

    #[tokio::test]
    async fn test_token_budget_usage() {
        let mut budget = TokenBudget {
            total_budget: 1_000_000,
            used: 0,
            warning_threshold: 800_000,
            per_agent_usage: HashMap::new(),
        };

        // Simulate usage
        budget.used += 50_000;
        *budget
            .per_agent_usage
            .entry("agent-1".to_string())
            .or_insert(0) += 50_000;

        assert_eq!(budget.used, 50_000);
        assert_eq!(budget.per_agent_usage.get("agent-1"), Some(&50_000));

        // Add more usage
        budget.used += 30_000;
        *budget
            .per_agent_usage
            .entry("agent-2".to_string())
            .or_insert(0) += 30_000;

        assert_eq!(budget.used, 80_000);
        assert_eq!(budget.per_agent_usage.get("agent-2"), Some(&30_000));
        assert_eq!(budget.per_agent_usage.len(), 2);
    }

    #[tokio::test]
    async fn test_agent_info_tracking() {
        let mut agents = HashMap::new();

        let agent1 = AgentInfo {
            agent_id: "agent-1".to_string(),
            agent_type: "code-reviewer".to_string(),
            status: "active".to_string(),
            last_heartbeat: "2025-11-01T00:00:00Z".to_string(),
        };

        agents.insert("agent-1".to_string(), agent1.clone());

        assert_eq!(agents.len(), 1);
        assert_eq!(agents.get("agent-1").unwrap().agent_id, "agent-1");
        assert_eq!(agents.get("agent-1").unwrap().status, "active");
    }

    #[tokio::test]
    async fn test_idempotency_entry_expiration() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(600);
        let past = now - Duration::from_secs(1);

        let valid_entry = IdempotencyEntry {
            response: RpcResponse {
                id: "test-1".to_string(),
                result: Some(json!({"status": "ok"})),
                error: None,
            },
            expires_at: future,
        };

        let expired_entry = IdempotencyEntry {
            response: RpcResponse {
                id: "test-2".to_string(),
                result: Some(json!({"status": "ok"})),
                error: None,
            },
            expires_at: past,
        };

        assert!(valid_entry.expires_at > now);
        assert!(expired_entry.expires_at < now);
    }

    #[tokio::test]
    async fn test_rpc_response_success() {
        let response = RpcResponse {
            id: "req-123".to_string(),
            result: Some(json!({"success": true, "data": "test"})),
            error: None,
        };

        assert_eq!(response.id, "req-123");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_rpc_response_error() {
        let response = RpcResponse {
            id: "req-456".to_string(),
            result: None,
            error: Some(RpcError {
                code: ERROR_INVALID_PARAMS,
                message: "Invalid parameters".to_string(),
                data: None,
            }),
        };

        assert_eq!(response.id, "req-456");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.as_ref().unwrap().code, ERROR_INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_task_queue_capacity() {
        let (tx, _rx) = async_channel::bounded(10);

        // Queue should accept items up to capacity
        for i in 0..10 {
            let request = RpcRequest {
                id: format!("req-{}", i),
                idem_key: None,
                method: "test.method".to_string(),
                params: json!({}),
            };
            assert!(tx.try_send(request).is_ok());
        }

        // 11th item should fail (queue full)
        let overflow_request = RpcRequest {
            id: "req-overflow".to_string(),
            idem_key: None,
            method: "test.method".to_string(),
            params: json!({}),
        };
        assert!(tx.try_send(overflow_request).is_err());
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ERROR_INVALID_PARAMS, -32602);
        assert_eq!(ERROR_METHOD_NOT_FOUND, -32601);
        assert_eq!(ERROR_BACKPRESSURE, 429);
        assert_eq!(ERROR_CONFLICT, 409);
    }
}
