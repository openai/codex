use anyhow::Context as _;
use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::extract::ws::Message as WsMessage;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::Html;
use axum::response::IntoResponse as _;
use axum::response::Response;
use axum::routing::get;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::SinkExt as _;
use futures::StreamExt as _;
use include_dir::Dir;
use rand::RngCore as _;
use serde::Deserialize;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt as _;
use tokio::io::AsyncWriteExt as _;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::debug;
use tracing::info;
use tracing::warn;
use url::Url;

static UI_DIST: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/ui/dist");

#[derive(Debug, Clone)]
pub struct CompanionOptions {
    /// Port to bind on localhost. Use 0 to pick an ephemeral port.
    pub port: u16,
    /// When true, attempt to open a browser to the Companion URL.
    pub open_browser: bool,
    /// Optional UI URL used for local frontend development with HMR.
    pub ui_dev_url: Option<String>,
    /// Raw `-c key=value` overrides to forward to the spawned `codex app-server` process.
    pub config_overrides: Vec<String>,
    /// Optional initial prompt to pass through to the web UI.
    pub initial_prompt: Option<String>,
}

impl CompanionOptions {
    pub fn new(
        port: u16,
        open_browser: bool,
        ui_dev_url: Option<String>,
        config_overrides: Vec<String>,
        initial_prompt: Option<String>,
    ) -> Self {
        Self {
            port,
            open_browser,
            ui_dev_url,
            config_overrides,
            initial_prompt,
        }
    }
}

#[derive(Debug)]
struct AppState {
    token: String,
    stdout_tx: broadcast::Sender<String>,
    stdin_tx: mpsc::Sender<String>,
    ws_slots: Arc<Semaphore>,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

async fn ui_index(Query(q): Query<TokenQuery>, State(state): State<Arc<AppState>>) -> Response {
    if q.token.as_deref() != Some(state.token.as_str()) {
        return (StatusCode::UNAUTHORIZED, "Missing or invalid token.\n").into_response();
    }

    let Some(file) = UI_DIST.get_file("index.html") else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Missing embedded UI assets.\n",
        )
            .into_response();
    };

    let html = match std::str::from_utf8(file.contents()) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Embedded UI is not valid UTF-8.\n",
            )
                .into_response();
        }
    };

    let mut resp = Html(html).into_response();
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    resp
}

fn asset_response(path: &str, cache_control: &'static str) -> Response {
    let Some(file) = UI_DIST.get_file(path) else {
        return (StatusCode::NOT_FOUND, "Asset not found.\n").into_response();
    };

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = Response::new(Body::from(file.contents().to_vec()));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    response
}

async fn ui_assets(Path(path): Path<String>) -> Response {
    if path.is_empty() || path.contains("..") {
        return (StatusCode::BAD_REQUEST, "Invalid asset path.\n").into_response();
    }

    let normalized = format!("assets/{path}");
    asset_response(&normalized, "public, max-age=31536000, immutable")
}

async fn ws_route(
    Query(q): Query<TokenQuery>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> Response {
    if q.token.as_deref() != Some(state.token.as_str()) {
        return (StatusCode::UNAUTHORIZED, "Missing or invalid token.\n").into_response();
    }

    let Ok(permit) = state.ws_slots.clone().try_acquire_owned() else {
        return (StatusCode::CONFLICT, "Only one client is supported.\n").into_response();
    };

    ws.on_upgrade(move |socket| handle_ws(socket, state, permit))
        .into_response()
}

async fn handle_ws(
    socket: WebSocket,
    state: Arc<AppState>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let mut app_stdout = state.stdout_tx.subscribe();
    let stdout_fwd = tokio::spawn(async move {
        loop {
            match app_stdout.recv().await {
                Ok(line) => {
                    if ws_tx.send(WsMessage::Text(line.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    debug!("companion: lagged websocket receiver (skipped {skipped} messages)");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(msg) = ws_rx.next().await {
        let Ok(msg) = msg else {
            break;
        };
        match msg {
            WsMessage::Text(text) => {
                if state.stdin_tx.send(text.to_string()).await.is_err() {
                    break;
                }
            }
            WsMessage::Close(_) => break,
            WsMessage::Ping(_) => {}
            _ => {}
        }
    }

    stdout_fwd.abort();
}

fn generate_token() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_backend_origin(addr: SocketAddr) -> String {
    format!("http://127.0.0.1:{}", addr.port())
}

fn build_browser_url(
    addr: SocketAddr,
    token: &str,
    initial_prompt: Option<&str>,
    ui_dev_url: Option<&str>,
) -> anyhow::Result<String> {
    let backend_origin = build_backend_origin(addr);

    let mut url = match ui_dev_url {
        Some(dev_url) => Url::parse(dev_url)
            .with_context(|| format!("invalid --companion-ui-dev-url: {dev_url}"))?,
        None => Url::parse(&backend_origin)?,
    };
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("token", token);
        if ui_dev_url.is_some() {
            qp.append_pair("backend", &backend_origin);
        }
        if let Some(prompt) = initial_prompt
            && !prompt.trim().is_empty()
        {
            qp.append_pair("prompt", prompt);
        }
    }
    Ok(url.into())
}

async fn try_open_browser(url: &str) -> anyhow::Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("open");
        cmd.arg(url);
        cmd
    } else if cfg!(windows) {
        // `start` is a cmd.exe builtin; the empty title avoids treating the URL as the title.
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    } else {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    let status = cmd
        .status()
        .await
        .context("failed to spawn browser opener")?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("browser opener exited with status {status}");
    }
}

struct RunningCompanion {
    addr: SocketAddr,
    token: String,
    shutdown_tx: watch::Sender<bool>,
    child: Child,
    server_task: tokio::task::JoinHandle<anyhow::Result<()>>,
    io_tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl RunningCompanion {
    async fn shutdown(mut self) -> anyhow::Result<()> {
        let _ = self.shutdown_tx.send(true);

        if let Err(err) = self.child.kill().await {
            debug!("companion: failed to kill app-server child: {err}");
        }
        let _ = self.child.wait().await;

        for task in self.io_tasks {
            task.abort();
        }
        self.server_task.abort();

        Ok(())
    }
}

async fn start_companion(
    options: &CompanionOptions,
    child_program: PathBuf,
    child_args: Vec<String>,
) -> anyhow::Result<RunningCompanion> {
    let token = generate_token();

    let (stdout_tx, _stdout_rx) = broadcast::channel::<String>(512);
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(256);
    let ws_slots = Arc::new(Semaphore::new(1));

    let state = Arc::new(AppState {
        token: token.clone(),
        stdout_tx: stdout_tx.clone(),
        stdin_tx: stdin_tx.clone(),
        ws_slots,
    });

    let listener = tokio::net::TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        options.port,
    ))
    .await
    .context("failed to bind companion listener")?;
    let addr = listener.local_addr().context("failed to read local addr")?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Spawn the app-server child.
    let mut child_cmd = Command::new(&child_program);
    child_cmd.args(&child_args);
    child_cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = child_cmd.spawn().context("failed to spawn app-server")?;
    let child_stdin = child.stdin.take().context("app-server stdin unavailable")?;
    let child_stdout = child
        .stdout
        .take()
        .context("app-server stdout unavailable")?;
    let child_stderr = child
        .stderr
        .take()
        .context("app-server stderr unavailable")?;

    let mut io_tasks = Vec::new();

    // Task: forward WS -> child stdin.
    io_tasks.push(tokio::spawn({
        let mut shutdown_rx = shutdown_rx.clone();
        async move {
            let mut stdin = child_stdin;
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    msg = stdin_rx.recv() => {
                        let Some(msg) = msg else { break };
                        if stdin.write_all(msg.as_bytes()).await.is_err() { break; }
                        if stdin.write_all(b"\n").await.is_err() { break; }
                        let _ = stdin.flush().await;
                    }
                }
            }
        }
    }));

    // Task: forward child stdout -> WS broadcast.
    io_tasks.push(tokio::spawn({
        let mut shutdown_rx = shutdown_rx.clone();
        let stdout_tx = stdout_tx.clone();
        async move {
            let mut lines = BufReader::new(child_stdout).lines();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    next = lines.next_line() => {
                        let Ok(Some(line)) = next else { break };
                        let _ = stdout_tx.send(line);
                    }
                }
            }
        }
    }));

    // Task: log child stderr.
    io_tasks.push(tokio::spawn({
        let mut shutdown_rx = shutdown_rx.clone();
        async move {
            let mut lines = BufReader::new(child_stderr).lines();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    next = lines.next_line() => {
                        let Ok(Some(line)) = next else { break };
                        warn!("app-server: {line}");
                    }
                }
            }
        }
    }));

    let app = Router::new()
        .route("/", get(ui_index))
        .route("/assets/{*path}", get(ui_assets))
        .route("/ws", get(ws_route))
        .with_state(state);

    let server_task = tokio::spawn({
        let mut shutdown_rx = shutdown_rx;
        async move {
            info!("companion listening on http://{}", addr);
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
                .context("companion http server failed")?;
            Ok(())
        }
    });

    Ok(RunningCompanion {
        addr,
        token,
        shutdown_tx,
        child,
        server_task,
        io_tasks,
    })
}

/// Run the Codex Companion server and block until Ctrl-C.
///
/// This will:
/// - Spawn `codex app-server`
/// - Host a local web UI on `127.0.0.1`
/// - Bridge app-server JSON-RPC JSONL over a WebSocket
pub async fn run(options: CompanionOptions) -> anyhow::Result<()> {
    let codex_exe = std::env::current_exe().context("failed to resolve current executable")?;

    // Spawn `codex app-server` via the same binary, forwarding root `-c` overrides.
    let mut child_args = vec!["app-server".to_string()];
    for ov in &options.config_overrides {
        child_args.push("-c".to_string());
        child_args.push(ov.clone());
    }

    let running = start_companion(&options, codex_exe, child_args).await?;

    let url = build_browser_url(
        running.addr,
        &running.token,
        options.initial_prompt.as_deref(),
        options.ui_dev_url.as_deref(),
    )?;
    println!("Companion: {url}");
    if options.open_browser
        && let Err(err) = try_open_browser(&url).await
    {
        eprintln!("warning: failed to open browser: {err}");
    }

    tokio::signal::ctrl_c()
        .await
        .context("failed to install Ctrl-C handler")?;

    running.shutdown().await?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn companion_requires_token_for_index() {
        let opts = CompanionOptions::new(0, false, None, Vec::new(), None);
        let running = start_companion(
            &opts,
            // Spawn a long-lived child that does nothing meaningful; we will kill it in shutdown.
            PathBuf::from("sh"),
            vec!["-c".to_string(), "cat >/dev/null".to_string()],
        )
        .await
        .expect("start companion");

        let url = format!("http://127.0.0.1:{}/", running.addr.port());
        let resp = reqwest::get(url).await.expect("request");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        running.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn websocket_rejects_missing_token() {
        let opts = CompanionOptions::new(0, false, None, Vec::new(), None);
        let running = start_companion(
            &opts,
            PathBuf::from("sh"),
            vec!["-c".to_string(), "cat >/dev/null".to_string()],
        )
        .await
        .expect("start companion");

        let ws_url = format!("ws://127.0.0.1:{}/ws", running.addr.port());
        let res = tokio_tungstenite::connect_async(ws_url).await;
        assert!(res.is_err(), "expected connect to fail without token");

        running.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn websocket_accepts_token_and_bridges_echo() {
        let opts = CompanionOptions::new(0, false, None, Vec::new(), None);
        let running = start_companion(
            &opts,
            PathBuf::from("sh"),
            vec!["-c".to_string(), "cat".to_string()],
        )
        .await
        .expect("start companion");

        let ws_url = format!(
            "ws://127.0.0.1:{}/ws?token={}",
            running.addr.port(),
            running.token
        );
        let (mut ws, _resp) = tokio_tungstenite::connect_async(ws_url)
            .await
            .expect("connect");

        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            "{\"hello\":\"world\"}".to_string().into(),
        ))
        .await
        .expect("send");

        let msg = ws.next().await.expect("ws msg").expect("ws ok");
        let tokio_tungstenite::tungstenite::Message::Text(line) = msg else {
            panic!("expected text message");
        };
        assert_eq!(line, "{\"hello\":\"world\"}");

        running.shutdown().await.expect("shutdown");
    }

    #[test]
    fn build_browser_url_points_to_dev_ui_when_configured() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4321);
        let url = build_browser_url(
            addr,
            "token-123",
            Some("hello world"),
            Some("http://127.0.0.1:5173"),
        )
        .expect("build browser url");
        assert_eq!(
            url,
            "http://127.0.0.1:5173/?token=token-123&backend=http%3A%2F%2F127.0.0.1%3A4321&prompt=hello+world"
        );
    }
}
