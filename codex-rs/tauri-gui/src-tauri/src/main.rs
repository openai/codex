// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod codex_bridge;
mod commit_quality;
mod db;
mod events;
mod kernel_bridge;
mod orchestration;
mod shortcuts;
mod tray;
mod updater;
mod watcher;

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use tracing::{error, info};

// Application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<RwLock<db::Database>>,
    pub watcher_running: Arc<RwLock<bool>>,
}

// Orchestration state (separate from AppState)
pub struct OrchestratorState {
    pub orchestration: orchestration::OrchestrationState,
}

// Tauri commands
#[tauri::command]
async fn greet(name: &str) -> Result<String, String> {
    Ok(format!("Hello, {}! Welcome to Codex AI-Native OS!", name))
}

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let watcher_running = *state.watcher_running.read().await;

    // Check CUDA availability
    #[cfg(feature = "cuda")]
    let cuda_available = codex_cuda_runtime::is_cuda_available();
    #[cfg(not(feature = "cuda"))]
    let cuda_available = false;

    Ok(serde_json::json!({
        "core_status": "running",
        "watcher_status": if watcher_running { "running" } else { "stopped" },
        "version": env!("CARGO_PKG_VERSION"),
        "cuda_available": cuda_available,
        "gpu_acceleration": true
    }))
}

#[tauri::command]
async fn get_gpu_stats() -> Result<serde_json::Value, String> {
    #[cfg(feature = "cuda")]
    {
        match codex_cuda_runtime::CudaRuntime::new(0) {
            Ok(cuda) => {
                match cuda.get_device_info() {
                    Ok(info) => Ok(serde_json::json!({
                        "available": true,
                        "device_name": info.name,
                        "compute_capability": format!("{}.{}", info.compute_capability_major, info.compute_capability_minor),
                        "total_memory": info.total_memory,
                    })),
                    Err(e) => Err(format!("Failed to get device info: {e}"))
                }
            }
            Err(e) => Err(format!("CUDA not available: {e}"))
        }
    }
    
    #[cfg(not(feature = "cuda"))]
    {
        Ok(serde_json::json!({
            "available": false,
            "message": "CUDA not compiled (compile with --features cuda)"
        }))
    }
}

#[tauri::command]
async fn start_file_watcher(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    workspace_path: String,
) -> Result<(), String> {
    let mut watcher_running = state.watcher_running.write().await;

    if *watcher_running {
        return Err("Watcher is already running".to_string());
    }

    info!("Starting file watcher for: {}", workspace_path);

    // Start watcher in background
    let state_clone = state.inner().clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        if let Err(e) = watcher::start_watcher(&workspace_path, state_clone, app_clone).await {
            error!("File watcher error: {}", e);
        }
    });

    *watcher_running = true;
    Ok(())
}

#[tauri::command]
async fn stop_file_watcher(state: State<'_, AppState>) -> Result<(), String> {
    let mut watcher_running = state.watcher_running.write().await;
    *watcher_running = false;
    info!("File watcher stopped");
    Ok(())
}

#[tauri::command]
async fn get_recent_changes(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.read().await;
    db.get_recent_changes(limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn enable_autostart(enabled: bool) -> Result<(), String> {
    autostart::set_autostart(enabled).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,codex_tauri=debug")
        .init();

    info!("Starting Codex AI-Native OS...");

    // Initialize database
    let db = db::Database::new()
        .await
        .expect("Failed to initialize database");

    let app_state = AppState {
        db: Arc::new(RwLock::new(db)),
        watcher_running: Arc::new(RwLock::new(false)),
    };

    let orchestrator_state = OrchestratorState {
        orchestration: orchestration::OrchestrationState::new(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .manage(orchestrator_state)
        .setup(|app| {
            // Setup system tray
            tray::create_tray(app.handle())?;

            // Setup global shortcuts
            shortcuts::setup_shortcuts(app.handle())?;

            // Check for updates (async)
            let app_handle = app.handle().clone();
            tokio::spawn(async move {
                updater::check_for_updates(&app_handle).await;
            });

            info!("Application initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_status,
            get_gpu_stats,
            start_file_watcher,
            stop_file_watcher,
            get_recent_changes,
            enable_autostart,
            codex_bridge::codex_create_blueprint,
            codex_bridge::codex_execute_blueprint,
            codex_bridge::codex_list_blueprints,
            codex_bridge::codex_research,
            codex_bridge::codex_list_mcp_tools,
            codex_bridge::codex_invoke_mcp_tool,
            updater::manual_update_check,
            kernel_bridge::kernel_get_status,
            kernel_bridge::kernel_optimize_process,
            kernel_bridge::kernel_allocate_memory,
            kernel_bridge::kernel_free_memory,
            commit_quality::analyze_commit_quality,
            commit_quality::analyze_commits_batch,
            orchestration::orchestrate_parallel,
            orchestration::get_orchestration_progress,
            orchestration::compare_agent_results,
            orchestration::create_worktree,
            orchestration::list_worktrees,
            orchestration::remove_worktree,
            orchestration::merge_worktree,
            orchestration::get_resource_capacity,
            orchestration::get_system_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
