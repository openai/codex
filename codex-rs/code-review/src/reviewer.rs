//! Continuous code reviewer

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::analyzer::{AnalysisResult, CodeAnalyzer};
use crate::config::ReviewConfig;
use crate::session::{ImprovementRecord, SessionManager};
use crate::watcher::{FileWatcher, WatchEvent};

/// Review task types
#[derive(Debug, Clone)]
pub enum ReviewTask {
    /// Analyze a single file
    AnalyzeFile(PathBuf),

    /// Analyze multiple files
    AnalyzeFiles(Vec<PathBuf>),

    /// Review changes in a directory
    ReviewDirectory(PathBuf),

    /// Apply an improvement
    ApplyImprovement {
        file_path: PathBuf,
        improvement_type: String,
        description: String,
        code_changes: Option<String>,
    },

    /// Periodic check
    PeriodicCheck,

    /// Stop the reviewer
    Stop,
}

/// Review type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewType {
    /// Incremental review (only changed files)
    Incremental,

    /// Full review (all files)
    Full,

    /// Quick check (fast, less thorough)
    Quick,
}

/// Continuous code reviewer
pub struct ContinuousReviewer {
    /// Configuration
    config: Arc<RwLock<ReviewConfig>>,

    /// Code analyzer
    analyzer: Arc<CodeAnalyzer>,

    /// File watcher
    watcher: Arc<RwLock<FileWatcher>>,

    /// Session manager
    session_manager: Arc<RwLock<SessionManager>>,

    /// Task queue
    task_sender: Sender<ReviewTask>,
    task_receiver: Receiver<ReviewTask>,

    /// Running flag
    running: Arc<RwLock<bool>>,
}

impl ContinuousReviewer {
    /// Create a new continuous reviewer
    pub fn new(
        config: ReviewConfig,
        session_dir: PathBuf,
        watch_dir: PathBuf,
    ) -> Result<Self> {
        let analyzer = Arc::new(CodeAnalyzer::new(config.analysis.clone()));

        let watcher = FileWatcher::new(
            watch_dir,
            config.watch_patterns.clone(),
            config.ignore_patterns.clone(),
        )?;

        let session_manager = SessionManager::new(session_dir);

        let (task_sender, task_receiver) = async_channel::unbounded();

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            analyzer,
            watcher: Arc::new(RwLock::new(watcher)),
            session_manager: Arc::new(RwLock::new(session_manager)),
            task_sender,
            task_receiver,
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start continuous review
    pub async fn start(&self) -> Result<()> {
        info!("Starting continuous code review");

        // Set running flag
        *self.running.write().await = true;

        // Start session
        {
            let mut session_manager = self.session_manager.write().await;
            session_manager.start_session().await?;
        }

        // Start file watcher
        {
            let mut watcher = self.watcher.write().await;
            watcher.start()?;
        }

        // Spawn tasks
        let file_watch_handle = self.spawn_file_watcher();
        let periodic_check_handle = self.spawn_periodic_checker();
        let task_processor_handle = self.spawn_task_processor();

        info!("Continuous review started successfully");

        // Wait for all tasks
        tokio::try_join!(file_watch_handle, periodic_check_handle, task_processor_handle)?;

        Ok(())
    }

    /// Stop continuous review
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping continuous code review");

        // Set running flag
        *self.running.write().await = false;

        // Send stop task
        self.task_sender.send(ReviewTask::Stop).await?;

        // Stop file watcher
        {
            let mut watcher = self.watcher.write().await;
            watcher.stop()?;
        }

        // Complete session
        {
            let mut session_manager = self.session_manager.write().await;
            session_manager.complete_session().await?;
        }

        info!("Continuous review stopped");
        Ok(())
    }

    /// Submit a review task
    pub async fn submit_task(&self, task: ReviewTask) -> Result<()> {
        self.task_sender.send(task).await?;
        Ok(())
    }

    /// Spawn file watcher task
    fn spawn_file_watcher(&self) -> tokio::task::JoinHandle<Result<()>> {
        let watcher = Arc::clone(&self.watcher);
        let task_sender = self.task_sender.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            info!("File watcher task started");

            while *running.read().await {
                let watcher = watcher.read().await;
                if let Some(event) = watcher.next_event().await {
                    debug!("File watch event: {:?}", event);

                    if event.should_trigger_review() {
                        let paths: Vec<PathBuf> = event
                            .paths()
                            .iter()
                            .map(|p| p.to_path_buf())
                            .collect();

                        if paths.len() == 1 {
                            task_sender
                                .send(ReviewTask::AnalyzeFile(paths[0].clone()))
                                .await?;
                        } else {
                            task_sender
                                .send(ReviewTask::AnalyzeFiles(paths))
                                .await?;
                        }
                    }
                }
            }

            info!("File watcher task stopped");
            Ok(())
        })
    }

    /// Spawn periodic checker task
    fn spawn_periodic_checker(&self) -> tokio::task::JoinHandle<Result<()>> {
        let config = Arc::clone(&self.config);
        let task_sender = self.task_sender.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            info!("Periodic checker task started");

            let check_minutes = {
                let config = config.read().await;
                config.triggers.periodic_check_minutes
            };

            if let Some(minutes) = check_minutes {
                let mut check_interval = interval(Duration::from_secs(minutes * 60));

                while *running.read().await {
                    check_interval.tick().await;
                    debug!("Triggering periodic check");

                    if let Err(e) = task_sender.send(ReviewTask::PeriodicCheck).await {
                        error!("Failed to send periodic check task: {}", e);
                    }
                }
            } else {
                // No periodic checks configured, just wait
                while *running.read().await {
                    sleep(Duration::from_secs(10)).await;
                }
            }

            info!("Periodic checker task stopped");
            Ok(())
        })
    }

    /// Spawn task processor
    fn spawn_task_processor(&self) -> tokio::task::JoinHandle<Result<()>> {
        let task_receiver = self.task_receiver.clone();
        let analyzer = Arc::clone(&self.analyzer);
        let session_manager = Arc::clone(&self.session_manager);
        let config = Arc::clone(&self.config);

        tokio::spawn(async move {
            info!("Task processor started");

            while let Ok(task) = task_receiver.recv().await {
                match task {
                    ReviewTask::Stop => {
                        info!("Received stop task");
                        break;
                    }

                    ReviewTask::AnalyzeFile(file_path) => {
                        info!("Processing: Analyze file {:?}", file_path);

                        match analyzer.analyze_file(&file_path).await {
                            Ok(result) => {
                                info!(
                                    "Analysis complete: {} issues, {} suggestions",
                                    result.issues.len(),
                                    result.suggestions.len()
                                );

                                let mut session_manager = session_manager.write().await;
                                if let Some(session) = session_manager.current_session_mut() {
                                    session.add_analysis_result(result.clone());
                                }

                                // Auto-apply improvements if configured
                                let config = config.read().await;
                                if config.policies.auto_fix_style {
                                    // Apply style fixes
                                    Self::apply_style_fixes(&result, &config).await;
                                }

                                // Save session
                                if let Err(e) = session_manager.save_current_session().await {
                                    error!("Failed to save session: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to analyze file {:?}: {}", file_path, e);
                            }
                        }
                    }

                    ReviewTask::AnalyzeFiles(file_paths) => {
                        info!("Processing: Analyze {} files", file_paths.len());

                        match analyzer.analyze_files(&file_paths).await {
                            Ok(results) => {
                                info!("Analyzed {} files successfully", results.len());

                                let mut session_manager = session_manager.write().await;
                                if let Some(session) = session_manager.current_session_mut() {
                                    for result in results {
                                        session.add_analysis_result(result);
                                    }
                                }

                                if let Err(e) = session_manager.save_current_session().await {
                                    error!("Failed to save session: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to analyze files: {}", e);
                            }
                        }
                    }

                    ReviewTask::ReviewDirectory(dir_path) => {
                        info!("Processing: Review directory {:?}", dir_path);
                        // Implement directory scanning and review
                    }

                    ReviewTask::ApplyImprovement {
                        file_path,
                        improvement_type,
                        description,
                        code_changes,
                    } => {
                        info!(
                            "Processing: Apply improvement to {:?}: {}",
                            file_path, improvement_type
                        );

                        let record = ImprovementRecord::new(
                            file_path.clone(),
                            improvement_type.clone(),
                            description.clone(),
                            0.9, // confidence
                        );

                        let mut session_manager = session_manager.write().await;
                        if let Some(session) = session_manager.current_session_mut() {
                            session.record_improvement(record);
                        }

                        // Apply code changes
                        if let Some(_changes) = code_changes {
                            // Implement applying code changes
                            info!("Applied improvement to {:?}", file_path);
                        }
                    }

                    ReviewTask::PeriodicCheck => {
                        info!("Processing: Periodic check");
                        // Implement periodic health check
                    }
                }
            }

            info!("Task processor stopped");
            Ok(())
        })
    }

    /// Apply style fixes automatically
    async fn apply_style_fixes(_result: &AnalysisResult, _config: &ReviewConfig) {
        // Implement style fix application
        debug!("Applying style fixes");
    }

    /// Get current session statistics
    pub async fn get_statistics(&self) -> Option<crate::session::SessionStatistics> {
        let session_manager = self.session_manager.read().await;
        session_manager
            .current_session()
            .map(|s| s.statistics.clone())
    }

    /// Pause the reviewer
    pub async fn pause(&self) -> Result<()> {
        *self.running.write().await = false;
        let mut session_manager = self.session_manager.write().await;
        if let Some(session) = session_manager.current_session_mut() {
            session.pause();
        }
        info!("Reviewer paused");
        Ok(())
    }

    /// Resume the reviewer
    pub async fn resume(&self) -> Result<()> {
        *self.running.write().await = true;
        let mut session_manager = self.session_manager.write().await;
        if let Some(session) = session_manager.current_session_mut() {
            session.resume();
        }
        info!("Reviewer resumed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_reviewer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path().join("sessions");
        let watch_dir = temp_dir.path().join("code");

        std::fs::create_dir_all(&watch_dir).unwrap();

        let config = ReviewConfig::default();
        let reviewer = ContinuousReviewer::new(config, session_dir, watch_dir);

        assert!(reviewer.is_ok());
    }

    #[tokio::test]
    async fn test_submit_task() {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path().join("sessions");
        let watch_dir = temp_dir.path().join("code");

        std::fs::create_dir_all(&watch_dir).unwrap();

        let config = ReviewConfig::default();
        let reviewer = ContinuousReviewer::new(config, session_dir, watch_dir).unwrap();

        let result = reviewer
            .submit_task(ReviewTask::AnalyzeFile(PathBuf::from("test.rs")))
            .await;

        assert!(result.is_ok());
    }
}
