import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "../styles/Blueprints.css";

interface Blueprint {
  id: string;
  description: string;
  status: string;
  created_at: string;
}

interface BlueprintProgressEvent {
  blueprint_id: string;
  status: string;
  progress: number;
  message: string;
}

function Blueprints() {
  const [blueprints, setBlueprints] = useState<Blueprint[]>([]);
  const [newBlueprintDesc, setNewBlueprintDesc] = useState("");
  const [selectedMode, setSelectedMode] = useState("orchestrated");
  const [executingBlueprintId, setExecutingBlueprintId] = useState<string | null>(null);
  const [executionProgress, setExecutionProgress] = useState(0);

  useEffect(() => {
    loadBlueprints();

    // Listen for blueprint progress events
    const unlisten = listen<BlueprintProgressEvent>("blueprint:progress", (event) => {
      const { blueprint_id: _blueprint_id, progress, status } = event.payload;
      setExecutionProgress(progress);
      
      if (status === "Completed" || status === "Failed") {
        setExecutingBlueprintId(null);
        loadBlueprints();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadBlueprints = async () => {
    try {
      const result = await invoke<Blueprint[]>("codex_list_blueprints");
      setBlueprints(result);
    } catch (error) {
      console.error("Failed to load blueprints:", error);
    }
  };

  const handleCreateBlueprint = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!newBlueprintDesc.trim()) {
      alert("Please enter a description");
      return;
    }

    try {
      const result = await invoke<Blueprint>("codex_create_blueprint", {
        description: newBlueprintDesc,
        mode: selectedMode,
      });
      
      setBlueprints([result, ...blueprints]);
      setNewBlueprintDesc("");
      alert("Blueprint created successfully!");
    } catch (error) {
      console.error("Failed to create blueprint:", error);
      alert(`Failed to create blueprint: ${error}`);
    }
  };

  const handleExecuteBlueprint = async (id: string) => {
    if (executingBlueprintId) {
      alert("Another blueprint is currently executing");
      return;
    }

    try {
      setExecutingBlueprintId(id);
      setExecutionProgress(0);
      
      const result = await invoke("codex_execute_blueprint", { id });
      console.log("Execution result:", result);
      
      alert("Blueprint execution started!");
      loadBlueprints();
    } catch (error) {
      console.error("Failed to execute blueprint:", error);
      alert(`Failed to execute blueprint: ${error}`);
      setExecutingBlueprintId(null);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case "pending":
        return "status-pending";
      case "approved":
        return "status-approved";
      case "executing":
        return "status-executing";
      case "completed":
        return "status-completed";
      case "failed":
        return "status-failed";
      default:
        return "";
    }
  };

  return (
    <div className="blueprints">
      <h1>Blueprints</h1>

      <div className="create-blueprint-section">
        <h2>Create New Blueprint</h2>
        <form onSubmit={handleCreateBlueprint} className="create-form">
          <div className="form-group">
            <textarea
              placeholder="Blueprint description (e.g., 'Implement user authentication with JWT')"
              value={newBlueprintDesc}
              onChange={(e) => setNewBlueprintDesc(e.target.value)}
              className="blueprint-textarea"
              rows={3}
            />
          </div>
          
          <div className="form-group">
            <label>Mode:</label>
            <select
              value={selectedMode}
              onChange={(e) => setSelectedMode(e.target.value)}
              className="mode-select"
            >
              <option value="orchestrated">Orchestrated (AI agents)</option>
              <option value="simple">Simple</option>
              <option value="research">Research-based</option>
            </select>
          </div>
          
          <button type="submit" className="btn btn-primary">
            Create Blueprint
          </button>
        </form>
      </div>

      {executingBlueprintId && (
        <div className="execution-progress">
          <h3>Executing Blueprint: {executingBlueprintId}</h3>
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${executionProgress}%` }}
            />
          </div>
          <p className="progress-text">{executionProgress.toFixed(0)}% complete</p>
        </div>
      )}

      <div className="blueprints-list-section">
        <h2>Your Blueprints</h2>
        
        {blueprints.length === 0 ? (
          <p className="no-blueprints">
            No blueprints yet. Create your first blueprint to get started!
          </p>
        ) : (
          <div className="blueprints-grid">
            {blueprints.map((blueprint) => (
              <div key={blueprint.id} className="blueprint-card">
                <div className="blueprint-header">
                  <span className={`blueprint-status ${getStatusColor(blueprint.status)}`}>
                    {blueprint.status}
                  </span>
                  <span className="blueprint-id">{blueprint.id}</span>
                </div>
                
                <p className="blueprint-description">{blueprint.description}</p>
                
                <div className="blueprint-footer">
                  <span className="blueprint-created">
                    {new Date(blueprint.created_at).toLocaleString()}
                  </span>
                  
                  <div className="blueprint-actions">
                    {blueprint.status === "Approved" && (
                      <button
                        onClick={() => handleExecuteBlueprint(blueprint.id)}
                        className="btn btn-execute"
                        disabled={!!executingBlueprintId}
                      >
                        Execute
                      </button>
                    )}
                    {blueprint.status === "Pending" && (
                      <button className="btn btn-secondary" disabled>
                        Pending Approval
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default Blueprints;

