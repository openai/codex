import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import StatusCard from "../components/StatusCard";
import RecentChanges from "../components/RecentChanges";
import KernelStatus from "../components/KernelStatus";
import "../styles/Dashboard.css";

interface Status {
  core_status: string;
  watcher_status: string;
  version: string;
}

interface FileChangeEvent {
  file_path: string;
  change_type: string;
  lines_added: number;
  lines_removed: number;
}

function Dashboard() {
  const [status, setStatus] = useState<Status | null>(null);
  const [recentChanges, setRecentChanges] = useState<any[]>([]);
  const [workspacePath, setWorkspacePath] = useState("");
  const [watcherRunning, setWatcherRunning] = useState(false);

  useEffect(() => {
    // Load initial status
    loadStatus();
    loadRecentChanges();

    // Listen for file changes
    const unlisten = listen<FileChangeEvent>("file:changed", (event) => {
      console.log("File changed:", event.payload);
      loadRecentChanges();
    });

    // Poll status every 5 seconds
    const interval = setInterval(() => {
      loadStatus();
    }, 5000);

    return () => {
      unlisten.then((fn) => fn());
      clearInterval(interval);
    };
  }, []);

  const loadStatus = async () => {
    try {
      const result = await invoke<Status>("get_status");
      setStatus(result);
      setWatcherRunning(result.watcher_status === "running");
    } catch (error) {
      console.error("Failed to load status:", error);
    }
  };

  const loadRecentChanges = async () => {
    try {
      const changes = await invoke<any[]>("get_recent_changes", { limit: 20 });
      setRecentChanges(changes);
    } catch (error) {
      console.error("Failed to load recent changes:", error);
    }
  };

  const handleStartWatcher = async () => {
    if (!workspacePath) {
      alert("Please enter a workspace path");
      return;
    }

    try {
      await invoke("start_file_watcher", { workspacePath });
      setWatcherRunning(true);
      loadStatus();
    } catch (error) {
      console.error("Failed to start watcher:", error);
      alert(`Failed to start watcher: ${error}`);
    }
  };

  const handleStopWatcher = async () => {
    try {
      await invoke("stop_file_watcher");
      setWatcherRunning(false);
      loadStatus();
    } catch (error) {
      console.error("Failed to stop watcher:", error);
    }
  };

  const handleNewBlueprint = async () => {
    const description = prompt("Blueprint description:");
    if (!description) return;

    try {
      const result = await invoke("codex_create_blueprint", {
        description,
        mode: "orchestrated",
      });
      console.log("Blueprint created:", result);
      alert("Blueprint created successfully!");
    } catch (error) {
      console.error("Failed to create blueprint:", error);
      alert(`Failed to create blueprint: ${error}`);
    }
  };

  const handleResearch = async () => {
    const query = prompt("Research query:");
    if (!query) return;

    try {
      const result = await invoke("codex_research", {
        query,
        depth: 3,
      });
      console.log("Research result:", result);
      alert("Research completed! Check console for results.");
    } catch (error) {
      console.error("Failed to perform research:", error);
      alert(`Failed to perform research: ${error}`);
    }
  };

  return (
    <div className="dashboard">
      <h1>Dashboard</h1>

      <div className="status-grid">
        <StatusCard
          title="Codex Core"
          status={status?.core_status || "unknown"}
          icon="🔄"
        />
        <StatusCard
          title="File Watcher"
          status={status?.watcher_status || "stopped"}
          icon="👁️"
        />
        <StatusCard
          title="Version"
          status={status?.version || "0.1.0"}
          icon="📦"
        />
      </div>

      <div className="watcher-control">
        <h2>File System Watcher</h2>
        <div className="control-group">
          <input
            type="text"
            placeholder="Workspace path (e.g., C:\Users\...\project)"
            value={workspacePath}
            onChange={(e) => setWorkspacePath(e.target.value)}
            disabled={watcherRunning}
            className="input-field"
          />
          {watcherRunning ? (
            <button onClick={handleStopWatcher} className="btn btn-danger">
              Stop Monitoring
            </button>
          ) : (
            <button onClick={handleStartWatcher} className="btn btn-primary">
              Start Monitoring
            </button>
          )}
        </div>
      </div>

      <div className="quick-actions">
        <h2>Quick Actions</h2>
        <div className="action-buttons">
          <button onClick={handleNewBlueprint} className="btn btn-action">
            📋 New Blueprint
          </button>
          <button onClick={handleResearch} className="btn btn-action">
            🔍 Deep Research
          </button>
          <button className="btn btn-action" disabled>
            🔧 Run MCP Tool
          </button>
        </div>
      </div>

      <KernelStatus />

      <RecentChanges changes={recentChanges} />
    </div>
  );
}

export default Dashboard;

