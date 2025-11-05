use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use axum::Json;
use axum::Router;
use chrono::DateTime;
use chrono::Utc;
use http::Method;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::signal;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), GuiError> {
    init_tracing();

    let port = std::env::var("CODEX_GUI_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(8787);

    let cli_path = std::env::var("CODEX_GUI_CLI_PATH").unwrap_or_else(|_| "codex".to_string());

    let state = AppState::new(cli_path, action_definitions());
    let cli_path_for_log = state.cli_path.clone();

    let app = Router::new()
        .route("/api/actions", get(list_actions))
        .route("/api/actions/:id/execute", post(execute_action))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
                .allow_origin(Any),
        )
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.map_err(GuiError::from)?;

    info!(
        port,
        cli_path = cli_path_for_log.as_str(),
        "listening on http://0.0.0.0:{port}"
    );

    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());

    server.await.map_err(GuiError::from)
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[derive(Clone)]
struct AppState {
    cli_path: Arc<String>,
    actions: Arc<Vec<ActionDefinition>>,
}

impl AppState {
    fn new(cli_path: String, actions: Vec<ActionDefinition>) -> Self {
        Self {
            cli_path: Arc::new(cli_path),
            actions: Arc::new(actions),
        }
    }

    fn find_action(&self, id: &str) -> Option<ActionDefinition> {
        self.actions.iter().cloned().find(|action| action.id == id)
    }
}

async fn list_actions(State(state): State<AppState>) -> Json<Vec<ActionMetadata>> {
    let payload = state.actions.iter().map(ActionMetadata::from).collect();

    Json(payload)
}

async fn execute_action(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<ExecuteRequest>,
) -> Result<Json<ExecutionResponse>, GuiError> {
    let action = state
        .find_action(&id)
        .ok_or_else(|| GuiError::ActionNotFound(id.clone()))?;

    let args = action.build_args(&payload.values)?;

    let started_at = Instant::now();
    let output = Command::new(state.cli_path.as_str())
        .args(&args)
        .output()
        .await
        .map_err(GuiError::CommandIo)?;
    let duration_ms = started_at.elapsed().as_millis();

    let status = if output.status.success() {
        ExecutionStatus::Completed
    } else {
        warn!(action = action.id, status = ?output.status, "action failed");
        ExecutionStatus::Failed
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    let response = ExecutionResponse {
        id: Uuid::new_v4(),
        action_id: action.id.to_string(),
        command: std::iter::once(state.cli_path.as_str().to_string())
            .chain(args.into_iter())
            .collect(),
        executed_at: Utc::now(),
        duration_ms,
        status,
        exit_code: output.status.code(),
        stdout,
        stderr,
    };

    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteRequest {
    values: HashMap<String, String>,
}

#[derive(Clone)]
struct ActionDefinition {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: ActionCategory,
    cta_label: &'static str,
    fields: Vec<ActionFieldDefinition>,
}

impl ActionDefinition {
    fn build_args(&self, values: &HashMap<String, String>) -> Result<Vec<String>, GuiError> {
        match self.id {
            "ask" => {
                let prompt = self.required_value(values, "prompt")?;
                Ok(vec!["ask".to_string(), prompt])
            }
            "delegate" => {
                let agent = self.required_value(values, "agent")?;
                let goal = self.required_value(values, "goal")?;
                let mut args = vec!["delegate".to_string(), agent, "--goal".to_string(), goal];
                if let Some(scope) = self.optional_value(values, "scope") {
                    args.push("--scope".to_string());
                    args.push(scope);
                }
                Ok(args)
            }
            "research" => {
                let topic = self.required_value(values, "topic")?;
                let depth = self.value_or_default(values, "depth");
                let breadth = self.value_or_default(values, "breadth");
                let mut args = vec!["research".to_string(), topic];
                args.push("--depth".to_string());
                args.push(depth);
                args.push("--breadth".to_string());
                args.push(breadth);
                Ok(args)
            }
            "review" => {
                let task = self.required_value(values, "task")?;
                Ok(vec!["review".to_string(), task])
            }
            "audit" => {
                let task = self.required_value(values, "task")?;
                Ok(vec!["audit".to_string(), task])
            }
            other => Err(GuiError::UnknownAction(other.to_string())),
        }
    }

    fn required_value(
        &self,
        values: &HashMap<String, String>,
        field_id: &str,
    ) -> Result<String, GuiError> {
        let value = self.value_or_default(values, field_id);
        if value.trim().is_empty() {
            return Err(GuiError::Validation {
                field: field_id.to_string(),
                message: "This field is required".to_string(),
            });
        }
        Ok(value)
    }

    fn value_or_default(&self, values: &HashMap<String, String>, field_id: &str) -> String {
        let provided = values
            .get(field_id)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());

        if let Some(value) = provided {
            return value;
        }

        self.fields
            .iter()
            .find(|field| field.id == field_id)
            .and_then(|field| field.default_value.map(ToString::to_string))
            .unwrap_or_default()
    }

    fn optional_value(&self, values: &HashMap<String, String>, field_id: &str) -> Option<String> {
        values
            .get(field_id)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
    }
}

#[derive(Clone)]
struct ActionFieldDefinition {
    id: &'static str,
    label: &'static str,
    kind: FieldKind,
    placeholder: Option<&'static str>,
    helper_text: Option<&'static str>,
    required: bool,
    default_value: Option<&'static str>,
    options: Vec<FieldOption>,
}

impl ActionFieldDefinition {
    fn text_area(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: FieldKind::TextArea,
            placeholder: None,
            helper_text: None,
            required: true,
            default_value: None,
            options: Vec::new(),
        }
    }

    fn text(id: &'static str, label: &'static str) -> Self {
        Self {
            id,
            label,
            kind: FieldKind::Text,
            placeholder: None,
            helper_text: None,
            required: true,
            default_value: None,
            options: Vec::new(),
        }
    }

    fn select(
        id: &'static str,
        label: &'static str,
        options: Vec<FieldOption>,
        default_value: Option<&'static str>,
    ) -> Self {
        Self {
            id,
            label,
            kind: FieldKind::Select,
            placeholder: None,
            helper_text: None,
            required: true,
            default_value,
            options,
        }
    }

    fn with_placeholder(mut self, placeholder: &'static str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    fn with_helper_text(mut self, helper_text: &'static str) -> Self {
        self.helper_text = Some(helper_text);
        self
    }

    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

#[derive(Clone, Serialize)]
struct FieldOption {
    value: &'static str,
    label: &'static str,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum FieldKind {
    Text,
    TextArea,
    Select,
}

#[derive(Clone, Copy)]
enum ActionCategory {
    Launchpad,
    Collaboration,
    Quality,
}

impl ActionCategory {
    fn as_str(&self) -> &'static str {
        match self {
            ActionCategory::Launchpad => "Launchpad",
            ActionCategory::Collaboration => "Collaboration",
            ActionCategory::Quality => "Quality",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionMetadata {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    category: &'static str,
    cta_label: &'static str,
    fields: Vec<ActionField>,
}

impl From<&ActionDefinition> for ActionMetadata {
    fn from(def: &ActionDefinition) -> Self {
        Self {
            id: def.id,
            label: def.label,
            description: def.description,
            category: def.category.as_str(),
            cta_label: def.cta_label,
            fields: def.fields.iter().map(ActionField::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionField {
    id: &'static str,
    label: &'static str,
    kind: FieldKind,
    placeholder: Option<&'static str>,
    helper_text: Option<&'static str>,
    required: bool,
    default_value: Option<&'static str>,
    options: Vec<FieldOption>,
}

impl From<&ActionFieldDefinition> for ActionField {
    fn from(def: &ActionFieldDefinition) -> Self {
        Self {
            id: def.id,
            label: def.label,
            kind: def.kind,
            placeholder: def.placeholder,
            helper_text: def.helper_text,
            required: def.required,
            default_value: def.default_value,
            options: def.options.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionResponse {
    id: Uuid,
    action_id: String,
    command: Vec<String>,
    executed_at: DateTime<Utc>,
    duration_ms: u128,
    status: ExecutionStatus,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ExecutionStatus {
    Completed,
    Failed,
}

fn action_definitions() -> Vec<ActionDefinition> {
    vec![
        ActionDefinition {
            id: "ask",
            label: "Ask an Agent",
            description:
                "Send a quick question or mention a specialized agent to get focused help.",
            category: ActionCategory::Collaboration,
            cta_label: "Send request",
            fields: vec![
                ActionFieldDefinition::text_area("prompt", "Task or question")
                    .with_placeholder("@code-reviewer Review the changes in src/main.rs"),
            ],
        },
        ActionDefinition {
            id: "delegate",
            label: "Delegate to Specialist",
            description:
                "Assign a scoped goal to a dedicated specialist agent with optional context.",
            category: ActionCategory::Collaboration,
            cta_label: "Delegate task",
            fields: vec![
                ActionFieldDefinition::select(
                    "agent",
                    "Agent",
                    vec![
                        FieldOption {
                            value: "code-reviewer",
                            label: "Code Reviewer",
                        },
                        FieldOption {
                            value: "security-expert",
                            label: "Security Expert",
                        },
                        FieldOption {
                            value: "docs-writer",
                            label: "Docs Writer",
                        },
                        FieldOption {
                            value: "test-writer",
                            label: "Test Writer",
                        },
                    ],
                    Some("code-reviewer"),
                ),
                ActionFieldDefinition::text_area("goal", "Delegated goal")
                    .with_placeholder("Audit the new login flow for edge cases"),
                ActionFieldDefinition::text("scope", "Repository scope")
                    .optional()
                    .with_placeholder("apps/auth/src"),
            ],
        },
        ActionDefinition {
            id: "research",
            label: "Deep Research",
            description:
                "Launch a deep-research session with controllable depth and breadth settings.",
            category: ActionCategory::Launchpad,
            cta_label: "Run research",
            fields: vec![
                ActionFieldDefinition::text_area("topic", "Research topic")
                    .with_placeholder("Compare performance of async runtimes for Rust services"),
                ActionFieldDefinition::select(
                    "depth",
                    "Depth",
                    vec![
                        FieldOption {
                            value: "2",
                            label: "Exploratory",
                        },
                        FieldOption {
                            value: "3",
                            label: "Balanced",
                        },
                        FieldOption {
                            value: "4",
                            label: "Comprehensive",
                        },
                        FieldOption {
                            value: "5",
                            label: "Exhaustive",
                        },
                    ],
                    Some("3"),
                )
                .with_helper_text("Controls how many iterative passes Codex performs."),
                ActionFieldDefinition::select(
                    "breadth",
                    "Breadth",
                    vec![
                        FieldOption {
                            value: "6",
                            label: "Focused (6 sources)",
                        },
                        FieldOption {
                            value: "8",
                            label: "Standard (8 sources)",
                        },
                        FieldOption {
                            value: "10",
                            label: "Broad (10 sources)",
                        },
                    ],
                    Some("8"),
                )
                .with_helper_text("Number of unique sources Codex should aggregate."),
            ],
        },
        ActionDefinition {
            id: "review",
            label: "Quick Review",
            description: "Summarize feedback on a patch or task using the review agent.",
            category: ActionCategory::Quality,
            cta_label: "Request review",
            fields: vec![ActionFieldDefinition::text_area("task", "Review scope")
                .with_placeholder("Review the diff in src/lib.rs for regressions")],
        },
        ActionDefinition {
            id: "audit",
            label: "Security Audit",
            description: "Run a targeted security audit with the sec-audit agent.",
            category: ActionCategory::Quality,
            cta_label: "Start audit",
            fields: vec![ActionFieldDefinition::text_area("task", "Audit focus")
                .with_placeholder("Inspect dependency updates for high severity CVEs")],
        },
    ]
}

#[derive(Debug, Error)]
enum GuiError {
    #[error("action `{0}` not found")]
    ActionNotFound(String),
    #[error("{message}")]
    Validation { field: String, message: String },
    #[error("action `{0}` is not supported")]
    UnknownAction(String),
    #[error("failed to run command: {0}")]
    CommandIo(#[from] std::io::Error),
}

impl IntoResponse for GuiError {
    fn into_response(self) -> Response {
        let (status, code, message, field) = match &self {
            GuiError::ActionNotFound(id) => (
                StatusCode::NOT_FOUND,
                "action_not_found",
                format!("Action `{id}` was not found"),
                None,
            ),
            GuiError::Validation { field, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                message.clone(),
                Some(field.clone()),
            ),
            GuiError::UnknownAction(id) => (
                StatusCode::NOT_IMPLEMENTED,
                "unsupported_action",
                format!("Action `{id}` is not supported yet"),
                None,
            ),
            GuiError::CommandIo(error) => (
                StatusCode::BAD_GATEWAY,
                "command_error",
                error.to_string(),
                None,
            ),
        };

        let body = Json(ErrorResponse {
            code,
            message,
            field,
        });

        (status, body).into_response()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    code: &'static str,
    message: String,
    field: Option<String>,
}
