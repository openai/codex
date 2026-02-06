//! Index health check, self-repair, and metrics collection.
//!
//! Provides tools for monitoring and maintaining index health.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::error::Result;
use crate::storage::VectorStore;
use crate::storage::sqlite::SqliteStore;

// ==== Health Check ====

/// Index health status.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Overall health state
    pub state: HealthState,
    /// Number of indexed chunks
    pub chunk_count: i64,
    /// Number of indexed files (from catalog)
    pub file_count: i32,
    /// Number of failed chunks
    pub failed_chunk_count: i32,
    /// Vector store connection status
    pub vector_store_ok: bool,
    /// SQLite connection status
    pub sqlite_ok: bool,
    /// FTS index status
    pub fts_index_ok: bool,
    /// Vector index status
    pub vector_index_ok: bool,
    /// Issues found during check
    pub issues: Vec<HealthIssue>,
    /// Check duration in milliseconds
    pub check_duration_ms: i64,
}

/// Overall health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    /// All checks passed
    Healthy,
    /// Some minor issues
    Degraded,
    /// Critical issues
    Unhealthy,
}

impl HealthState {
    #[allow(dead_code)]
    fn as_str(&self) -> &'static str {
        match self {
            HealthState::Healthy => "healthy",
            HealthState::Degraded => "degraded",
            HealthState::Unhealthy => "unhealthy",
        }
    }
}

/// Health issue found during check.
#[derive(Debug, Clone)]
pub struct HealthIssue {
    /// Issue severity
    pub severity: IssueSeverity,
    /// Issue category
    pub category: IssueCategory,
    /// Issue description
    pub message: String,
    /// Whether this issue can be auto-repaired
    pub repairable: bool,
}

/// Issue severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Error,
    Critical,
}

/// Issue category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    Database,
    Index,
    Storage,
    Consistency,
}

/// Index health checker.
pub struct HealthChecker {
    vector_store: Option<Arc<dyn VectorStore>>,
    sqlite_store: Option<Arc<SqliteStore>>,
}

impl HealthChecker {
    /// Create a new health checker.
    pub fn new() -> Self {
        Self {
            vector_store: None,
            sqlite_store: None,
        }
    }

    /// Set vector store for checking.
    pub fn with_vector_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(store);
        self
    }

    /// Set SQLite store for checking.
    pub fn with_sqlite(mut self, store: Arc<SqliteStore>) -> Self {
        self.sqlite_store = Some(store);
        self
    }

    /// Run health check.
    pub async fn check(&self) -> Result<HealthStatus> {
        let start = Instant::now();
        let mut issues = Vec::new();
        let mut vector_store_ok = false;
        let mut sqlite_ok = false;
        let mut fts_index_ok = false;
        let mut vector_index_ok = false;
        let mut chunk_count = 0i64;
        let mut file_count = 0i32;
        let mut failed_chunk_count = 0i32;

        // Check vector store
        if let Some(ref store) = self.vector_store {
            match store.count().await {
                Ok(count) => {
                    vector_store_ok = true;
                    chunk_count = count;
                }
                Err(e) => {
                    issues.push(HealthIssue {
                        severity: IssueSeverity::Critical,
                        category: IssueCategory::Database,
                        message: format!("Vector store count failed: {e}"),
                        repairable: false,
                    });
                }
            }

            // Check if table exists
            match store.table_exists().await {
                Ok(true) => {
                    fts_index_ok = true;
                    vector_index_ok = true;
                }
                Ok(false) => {
                    issues.push(HealthIssue {
                        severity: IssueSeverity::Error,
                        category: IssueCategory::Index,
                        message: "Vector store table does not exist".to_string(),
                        repairable: true,
                    });
                }
                Err(e) => {
                    issues.push(HealthIssue {
                        severity: IssueSeverity::Error,
                        category: IssueCategory::Index,
                        message: format!("Vector store table check failed: {e}"),
                        repairable: false,
                    });
                }
            }
        } else {
            issues.push(HealthIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Storage,
                message: "Vector store not configured".to_string(),
                repairable: false,
            });
        }

        // Check SQLite
        if let Some(ref store) = self.sqlite_store {
            match store
                .query(|conn| {
                    let count: i32 =
                        conn.query_row("SELECT COUNT(*) FROM catalog", [], |row| row.get(0))?;
                    let failed: i32 = conn.query_row(
                        "SELECT COALESCE(SUM(chunks_failed), 0) FROM catalog",
                        [],
                        |row| row.get(0),
                    )?;
                    Ok((count, failed))
                })
                .await
            {
                Ok((count, failed)) => {
                    sqlite_ok = true;
                    file_count = count;
                    failed_chunk_count = failed;

                    if failed > 0 {
                        issues.push(HealthIssue {
                            severity: IssueSeverity::Warning,
                            category: IssueCategory::Consistency,
                            message: format!("{failed} chunks failed to process"),
                            repairable: true,
                        });
                    }
                }
                Err(e) => {
                    issues.push(HealthIssue {
                        severity: IssueSeverity::Critical,
                        category: IssueCategory::Database,
                        message: format!("SQLite query failed: {e}"),
                        repairable: false,
                    });
                }
            }
        } else {
            issues.push(HealthIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Storage,
                message: "SQLite store not configured".to_string(),
                repairable: false,
            });
        }

        // Determine overall state
        let state = if issues.iter().any(|i| i.severity == IssueSeverity::Critical) {
            HealthState::Unhealthy
        } else if issues.iter().any(|i| i.severity == IssueSeverity::Error) {
            HealthState::Degraded
        } else if issues.iter().any(|i| i.severity == IssueSeverity::Warning) {
            HealthState::Degraded
        } else {
            HealthState::Healthy
        };

        Ok(HealthStatus {
            state,
            chunk_count,
            file_count,
            failed_chunk_count,
            vector_store_ok,
            sqlite_ok,
            fts_index_ok,
            vector_index_ok,
            issues,
            check_duration_ms: start.elapsed().as_millis() as i64,
        })
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ==== Self-Repair ====

/// Repair action result.
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// Whether repair was successful
    pub success: bool,
    /// Number of items repaired
    pub repaired_count: i32,
    /// Description of what was repaired
    pub message: String,
}

/// Index self-repair utilities.
pub struct IndexRepairer {
    vector_store: Option<Arc<dyn VectorStore>>,
    sqlite_store: Option<Arc<SqliteStore>>,
}

impl IndexRepairer {
    /// Create a new index repairer.
    pub fn new() -> Self {
        Self {
            vector_store: None,
            sqlite_store: None,
        }
    }

    /// Set vector store for repair.
    pub fn with_vector_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(store);
        self
    }

    /// Set SQLite store for repair.
    pub fn with_sqlite(mut self, store: Arc<SqliteStore>) -> Self {
        self.sqlite_store = Some(store);
        self
    }

    /// Repair a specific issue.
    pub async fn repair(&self, issue: &HealthIssue) -> Result<RepairResult> {
        if !issue.repairable {
            return Ok(RepairResult {
                success: false,
                repaired_count: 0,
                message: "Issue is not repairable".to_string(),
            });
        }

        match issue.category {
            IssueCategory::Index => self.repair_index().await,
            IssueCategory::Consistency => self.repair_consistency().await,
            _ => Ok(RepairResult {
                success: false,
                repaired_count: 0,
                message: "Repair not implemented for this category".to_string(),
            }),
        }
    }

    /// Repair index issues (recreate missing indices).
    async fn repair_index(&self) -> Result<RepairResult> {
        if let Some(ref store) = self.vector_store {
            if let Err(e) = store.create_fts_index().await {
                return Ok(RepairResult {
                    success: false,
                    repaired_count: 0,
                    message: format!("Failed to create FTS index: {e}"),
                });
            }

            if let Err(e) = store.create_vector_index().await {
                return Ok(RepairResult {
                    success: false,
                    repaired_count: 0,
                    message: format!("Failed to create vector index: {e}"),
                });
            }

            Ok(RepairResult {
                success: true,
                repaired_count: 2,
                message: "Recreated FTS and vector indices".to_string(),
            })
        } else {
            Ok(RepairResult {
                success: false,
                repaired_count: 0,
                message: "Vector store not configured".to_string(),
            })
        }
    }

    /// Repair consistency issues (mark failed chunks for reprocessing).
    async fn repair_consistency(&self) -> Result<RepairResult> {
        if let Some(ref store) = self.sqlite_store {
            let repaired = store
                .query(|conn| {
                    let count = conn.execute(
                        "UPDATE catalog SET chunks_failed = 0 WHERE chunks_failed > 0",
                        [],
                    )?;
                    Ok(count as i32)
                })
                .await?;

            Ok(RepairResult {
                success: true,
                repaired_count: repaired,
                message: format!("Reset {} files for reprocessing", repaired),
            })
        } else {
            Ok(RepairResult {
                success: false,
                repaired_count: 0,
                message: "SQLite store not configured".to_string(),
            })
        }
    }

    /// Run all repairable repairs.
    pub async fn repair_all(&self, status: &HealthStatus) -> Vec<RepairResult> {
        let mut results = Vec::new();
        for issue in &status.issues {
            if issue.repairable {
                if let Ok(result) = self.repair(issue).await {
                    results.push(result);
                }
            }
        }
        results
    }
}

impl Default for IndexRepairer {
    fn default() -> Self {
        Self::new()
    }
}

// ==== Metrics Collection ====

/// Index metrics.
#[derive(Debug, Clone, Default)]
pub struct IndexMetrics {
    /// Total indexed files
    pub total_files: i32,
    /// Total indexed chunks
    pub total_chunks: i64,
    /// Failed chunk count
    pub failed_chunks: i32,
    /// Storage size in bytes
    pub storage_bytes: i64,
    /// Last index time (Unix timestamp)
    pub last_indexed_at: i64,
    /// Index build time in milliseconds
    pub index_build_time_ms: i64,
    /// Average chunks per file
    pub avg_chunks_per_file: f32,
    /// Search latency samples (ms)
    pub search_latency_samples: Vec<i64>,
}

impl IndexMetrics {
    /// Calculate average search latency.
    pub fn avg_search_latency_ms(&self) -> f32 {
        if self.search_latency_samples.is_empty() {
            0.0
        } else {
            self.search_latency_samples.iter().sum::<i64>() as f32
                / self.search_latency_samples.len() as f32
        }
    }

    /// Calculate p99 search latency.
    pub fn p99_search_latency_ms(&self) -> i64 {
        if self.search_latency_samples.is_empty() {
            return 0;
        }
        let mut sorted = self.search_latency_samples.clone();
        sorted.sort();
        let idx = ((sorted.len() as f32) * 0.99).ceil() as usize - 1;
        sorted[idx.min(sorted.len() - 1)]
    }
}

/// Metrics collector for index operations.
pub struct MetricsCollector {
    sqlite_store: Option<Arc<SqliteStore>>,
    data_dir: Option<std::path::PathBuf>,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            sqlite_store: None,
            data_dir: None,
        }
    }

    /// Set SQLite store for metrics.
    pub fn with_sqlite(mut self, store: Arc<SqliteStore>) -> Self {
        self.sqlite_store = Some(store);
        self
    }

    /// Set data directory for storage size calculation.
    pub fn with_data_dir(mut self, path: &Path) -> Self {
        self.data_dir = Some(path.to_path_buf());
        self
    }

    /// Collect metrics.
    pub async fn collect(&self) -> Result<IndexMetrics> {
        let mut metrics = IndexMetrics::default();

        // Get file and chunk counts from SQLite
        if let Some(ref store) = self.sqlite_store {
            if let Ok((files, chunks, failed, last_indexed)) = store
                .query(|conn| {
                    let files: i32 =
                        conn.query_row("SELECT COUNT(*) FROM catalog", [], |row| row.get(0))?;
                    let chunks: i64 = conn.query_row(
                        "SELECT COALESCE(SUM(chunks_count), 0) FROM catalog",
                        [],
                        |row| row.get(0),
                    )?;
                    let failed: i32 = conn.query_row(
                        "SELECT COALESCE(SUM(chunks_failed), 0) FROM catalog",
                        [],
                        |row| row.get(0),
                    )?;
                    let last_indexed: i64 = conn.query_row(
                        "SELECT COALESCE(MAX(indexed_at), 0) FROM catalog",
                        [],
                        |row| row.get(0),
                    )?;
                    Ok((files, chunks, failed, last_indexed))
                })
                .await
            {
                metrics.total_files = files;
                metrics.total_chunks = chunks;
                metrics.failed_chunks = failed;
                metrics.last_indexed_at = last_indexed;

                if files > 0 {
                    metrics.avg_chunks_per_file = chunks as f32 / files as f32;
                }
            }
        }

        // Calculate storage size
        if let Some(ref path) = self.data_dir {
            metrics.storage_bytes = calculate_dir_size(path);
        }

        Ok(metrics)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate directory size recursively.
fn calculate_dir_size(path: &Path) -> i64 {
    let mut size = 0i64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                size += calculate_dir_size(&path);
            } else if let Ok(metadata) = path.metadata() {
                size += metadata.len() as i64;
            }
        }
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_health_state() {
        assert_eq!(HealthState::Healthy.as_str(), "healthy");
        assert_eq!(HealthState::Degraded.as_str(), "degraded");
        assert_eq!(HealthState::Unhealthy.as_str(), "unhealthy");
    }

    #[test]
    fn test_metrics_avg_latency() {
        let mut metrics = IndexMetrics::default();
        metrics.search_latency_samples = vec![10, 20, 30, 40, 50];

        assert_eq!(metrics.avg_search_latency_ms(), 30.0);
    }

    #[test]
    fn test_metrics_p99_latency() {
        let mut metrics = IndexMetrics::default();
        metrics.search_latency_samples = (1..=100).collect();

        assert_eq!(metrics.p99_search_latency_ms(), 99);
    }

    #[test]
    fn test_metrics_empty_latency() {
        let metrics = IndexMetrics::default();

        assert_eq!(metrics.avg_search_latency_ms(), 0.0);
        assert_eq!(metrics.p99_search_latency_ms(), 0);
    }

    #[test]
    fn test_calculate_dir_size() {
        let dir = TempDir::new().unwrap();

        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let size = calculate_dir_size(dir.path());
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_health_checker_no_stores() {
        let checker = HealthChecker::new();
        let status = checker.check().await.unwrap();

        assert!(!status.issues.is_empty());
        assert!(!status.vector_store_ok);
        assert!(!status.sqlite_ok);
    }

    #[tokio::test]
    async fn test_health_checker_with_sqlite() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());

        let checker = HealthChecker::new().with_sqlite(store);
        let status = checker.check().await.unwrap();

        assert!(status.sqlite_ok);
        assert_eq!(status.file_count, 0);
    }

    #[tokio::test]
    async fn test_repairer_non_repairable() {
        let repairer = IndexRepairer::new();
        let issue = HealthIssue {
            severity: IssueSeverity::Critical,
            category: IssueCategory::Database,
            message: "Test issue".to_string(),
            repairable: false,
        };

        let result = repairer.repair(&issue).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.repaired_count, 0);
    }

    #[tokio::test]
    async fn test_metrics_collector_with_sqlite() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());

        let collector = MetricsCollector::new()
            .with_sqlite(store)
            .with_data_dir(dir.path());

        let metrics = collector.collect().await.unwrap();

        assert_eq!(metrics.total_files, 0);
        assert_eq!(metrics.total_chunks, 0);
    }
}
