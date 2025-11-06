import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "../styles/Plans.css";

interface Plan {
  id: string;
  description: string;
  status: string;
  created_at: string;
}

interface PlanProgressEvent {
  Plan_id: string;
  status: string;
  progress: number;
  message: string;
}

function Plans() {
  const [Plans, setPlans] = useState<Plan[]>([]);
  const [newPlanDesc, setNewPlanDesc] = useState("");
  const [selectedMode, setSelectedMode] = useState("orchestrated");
  const [executingPlanId, setExecutingPlanId] = useState<string | null>(null);
  const [executionProgress, setExecutionProgress] = useState(0);

  useEffect(() => {
    loadPlans();

    // Listen for Plan progress events
    const unlisten = listen<PlanProgressEvent>("Plan:progress", (event) => {
      const { Plan_id: _Plan_id, progress, status } = event.payload;
      setExecutionProgress(progress);
      
      if (status === "Completed" || status === "Failed") {
        setExecutingPlanId(null);
        loadPlans();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const loadPlans = async () => {
    try {
      const result = await invoke<Plan[]>("codex_list_Plans");
      setPlans(result);
    } catch (error) {
      console.error("Failed to load Plans:", error);
    }
  };

  const handleCreatePlan = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!newPlanDesc.trim()) {
      alert("Please enter a description");
      return;
    }

    try {
      const result = await invoke<Plan>("codex_create_Plan", {
        description: newPlanDesc,
        mode: selectedMode,
      });
      
      setPlans([result, ...Plans]);
      setNewPlanDesc("");
      alert("Plan created successfully!");
    } catch (error) {
      console.error("Failed to create Plan:", error);
      alert(`Failed to create Plan: ${error}`);
    }
  };

  const handleExecutePlan = async (id: string) => {
    if (executingPlanId) {
      alert("Another Plan is currently executing");
      return;
    }

    try {
      setExecutingPlanId(id);
      setExecutionProgress(0);
      
      const result = await invoke("codex_execute_Plan", { id });
      console.log("Execution result:", result);
      
      alert("Plan execution started!");
      loadPlans();
    } catch (error) {
      console.error("Failed to execute Plan:", error);
      alert(`Failed to execute Plan: ${error}`);
      setExecutingPlanId(null);
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
    <div className="Plans">
      <h1>Plans</h1>

      <div className="create-Plan-section">
        <h2>Create New Plan</h2>
        <form onSubmit={handleCreatePlan} className="create-form">
          <div className="form-group">
            <textarea
              placeholder="Plan description (e.g., 'Implement user authentication with JWT')"
              value={newPlanDesc}
              onChange={(e) => setNewPlanDesc(e.target.value)}
              className="Plan-textarea"
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
            Create Plan
          </button>
        </form>
      </div>

      {executingPlanId && (
        <div className="execution-progress">
          <h3>Executing Plan: {executingPlanId}</h3>
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${executionProgress}%` }}
            />
          </div>
          <p className="progress-text">{executionProgress.toFixed(0)}% complete</p>
        </div>
      )}

      <div className="Plans-list-section">
        <h2>Your Plans</h2>
        
        {Plans.length === 0 ? (
          <p className="no-Plans">
            No Plans yet. Create your first Plan to get started!
          </p>
        ) : (
          <div className="Plans-grid">
            {Plans.map((Plan) => (
              <div key={Plan.id} className="Plan-card">
                <div className="Plan-header">
                  <span className={`Plan-status ${getStatusColor(Plan.status)}`}>
                    {Plan.status}
                  </span>
                  <span className="Plan-id">{Plan.id}</span>
                </div>
                
                <p className="Plan-description">{Plan.description}</p>
                
                <div className="Plan-footer">
                  <span className="Plan-created">
                    {new Date(Plan.created_at).toLocaleString()}
                  </span>
                  
                  <div className="Plan-actions">
                    {Plan.status === "Approved" && (
                      <button
                        onClick={() => handleExecutePlan(Plan.id)}
                        className="btn btn-execute"
                        disabled={!!executingPlanId}
                      >
                        Execute
                      </button>
                    )}
                    {Plan.status === "Pending" && (
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

export default Plans;

