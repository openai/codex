import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "../styles/KernelStatus.css";

interface GpuStatus {
  utilization: number;
  memory_used: number;
  memory_total: number;
  temperature: number;
}

interface MemoryPoolStatus {
  total_size: number;
  used_size: number;
  free_size: number;
  block_count: number;
  fragmentation_ratio: number;
}

interface SchedulerStats {
  ai_processes: number;
  scheduled_tasks: number;
  average_latency_ms: number;
}

interface KernelDriverStatus {
  loaded: boolean;
  version: string;
  gpu_status: GpuStatus | null;
  memory_pool: MemoryPoolStatus | null;
  scheduler_stats: SchedulerStats | null;
}

function KernelStatus() {
  const [status, setStatus] = useState<KernelDriverStatus | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadStatus();

    // Poll every 2 seconds
    const interval = setInterval(() => {
      loadStatus();
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  const loadStatus = async () => {
    try {
      const result = await invoke<KernelDriverStatus>("kernel_get_status");
      setStatus(result);
      setLoading(false);
    } catch (error) {
      console.error("Failed to load kernel status:", error);
      setLoading(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    const gb = bytes / (1024 * 1024 * 1024);
    return `${gb.toFixed(2)} GB`;
  };

  const formatMB = (bytes: number): string => {
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(0)} MB`;
  };

  if (loading) {
    return <div className="kernel-status loading">Loading kernel status...</div>;
  }

  if (!status) {
    return <div className="kernel-status error">Failed to load kernel status</div>;
  }

  return (
    <div className="kernel-status">
      <div className="kernel-header">
        <h2>AIネイティブOS - カーネル統合</h2>
        <div className={`driver-badge ${status.loaded ? "loaded" : "not-loaded"}`}>
          {status.loaded ? "✅ ドライバー起動中" : "❌ ドライバー未起動"}
        </div>
      </div>

      {!status.loaded && (
        <div className="driver-info">
          <p className="info-text">
            カーネルドライバーは現在利用できません。
          </p>
          <p className="info-text">
            AIネイティブOS機能を使用するには、管理者権限でドライバーをインストールしてください。
          </p>
          <button className="btn btn-secondary" disabled>
            ドライバーインストール（未実装）
          </button>
        </div>
      )}

      {status.loaded && (
        <>
          <div className="version-info">
            <span>Driver Version: {status.version}</span>
          </div>

          {status.gpu_status && (
            <div className="status-section">
              <h3>GPU Status</h3>
              <div className="status-grid">
                <div className="status-item">
                  <label>GPU使用率</label>
                  <div className="progress-bar">
                    <div
                      className="progress-fill gpu"
                      style={{ width: `${status.gpu_status.utilization}%` }}
                    />
                  </div>
                  <span className="value">{status.gpu_status.utilization.toFixed(1)}%</span>
                </div>

                <div className="status-item">
                  <label>GPU Memory</label>
                  <div className="progress-bar">
                    <div
                      className="progress-fill memory"
                      style={{
                        width: `${(status.gpu_status.memory_used / status.gpu_status.memory_total) * 100}%`,
                      }}
                    />
                  </div>
                  <span className="value">
                    {formatBytes(status.gpu_status.memory_used)} / {formatBytes(status.gpu_status.memory_total)}
                  </span>
                </div>

                <div className="status-item">
                  <label>Temperature</label>
                  <div className="temperature-display">
                    <span className="temp-value">{status.gpu_status.temperature.toFixed(1)}°C</span>
                    <div className={`temp-indicator ${status.gpu_status.temperature > 80 ? "hot" : ""}`} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {status.memory_pool && (
            <div className="status-section">
              <h3>AI Memory Pool</h3>
              <div className="status-grid">
                <div className="status-item">
                  <label>使用状況</label>
                  <div className="progress-bar">
                    <div
                      className="progress-fill pool"
                      style={{
                        width: `${(status.memory_pool.used_size / status.memory_pool.total_size) * 100}%`,
                      }}
                    />
                  </div>
                  <span className="value">
                    {formatMB(status.memory_pool.used_size)} / {formatMB(status.memory_pool.total_size)}
                  </span>
                </div>

                <div className="status-item">
                  <label>ブロック数</label>
                  <span className="value">{status.memory_pool.block_count.toLocaleString()}</span>
                </div>

                <div className="status-item">
                  <label>断片化率</label>
                  <span className="value">{(status.memory_pool.fragmentation_ratio * 100).toFixed(1)}%</span>
                </div>
              </div>
            </div>
          )}

          {status.scheduler_stats && (
            <div className="status-section">
              <h3>AI Scheduler</h3>
              <div className="status-grid">
                <div className="status-item">
                  <label>AI Processes</label>
                  <span className="value">{status.scheduler_stats.ai_processes}</span>
                </div>

                <div className="status-item">
                  <label>Scheduled Tasks</label>
                  <span className="value">{status.scheduler_stats.scheduled_tasks}</span>
                </div>

                <div className="status-item">
                  <label>Avg Latency</label>
                  <span className="value">{status.scheduler_stats.average_latency_ms.toFixed(2)} ms</span>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default KernelStatus;

