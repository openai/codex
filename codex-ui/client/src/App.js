import React, { useState, useEffect } from 'react';
import './App.css';
import ConfigManager from './ConfigManager'; // Import ConfigManager
import AgentsMdManager from './AgentsMdManager'; // Import AgentsMdManager
import CodexTaskManager from './CodexTaskManager'; // Import CodexTaskManager

function App() {
  const [apiKey, setApiKey] = useState('');
  const [projectDirectory, setProjectDirectory] = useState('');
  const [agentsFileContent, setAgentsFileContent] = useState('');
  // const [codexResponse, setCodexResponse] = useState(''); // This was already commented out/unused effectively
  const [isLoading, setIsLoading] = useState(false); // Initial loading for config
  const [error, setError] = useState(''); // Error for initial data fetching

  useEffect(() => {
    const fetchData = async () => {
      setIsLoading(true);
      setError(''); 
      try {
        // Fetch API Key
        const apiKeyRes = await fetch('/api/config/apikey');
        const apiKeyData = await apiKeyRes.json();
        if (apiKeyRes.ok) {
          setApiKey(apiKeyData.apiKey || '');
        } else {
          throw new Error(apiKeyData.error || `Failed to fetch API key (status: ${apiKeyRes.status})`);
        }

        // Fetch Project Directory
        const projectDirRes = await fetch('/api/config/projectdir');
        const projectDirData = await projectDirRes.json();
        if (projectDirRes.ok) {
          setProjectDirectory(projectDirData.projectDirectory || '');
        } else {
          throw new Error(projectDirData.error || `Failed to fetch project directory (status: ${projectDirRes.status})`);
        }

        // Fetch AGENTS.md content
        const agentsRes = await fetch('/api/agents'); // Backend handles the path for now
        const agentsData = await agentsRes.json();
        if (agentsRes.ok) {
          setAgentsFileContent(agentsData.content || '');
        } else {
          throw new Error(agentsData.error || `Failed to fetch AGENTS.md (status: ${agentsRes.status})`);
        }

      } catch (err) {
        setError(err.message);
        console.error("Fetch error:", err); 
      } finally {
        setIsLoading(false);
      }
    };
    fetchData();
  }, []);

  const handleApiKeyUpdate = (newApiKey) => {
    setApiKey(newApiKey);
  };

  const handleProjectDirUpdate = (newProjectDir) => {
    setProjectDirectory(newProjectDir);
  };

  const handleSaveAgentsMd = async (filePath, newContent) => {
    try {
      const response = await fetch('/api/agents', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: filePath, content: newContent }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data.error || `Failed to save AGENTS.md (status: ${response.status})`);
      }
      setAgentsFileContent(newContent); 
      return Promise.resolve(data.message || 'Content saved successfully.');
    } catch (err) {
      console.error("Save AGENTS.md error:", err);
      return Promise.reject(err);
    }
  };

  const handleCodexExecute = async (prompt, options) => {
    // This function will be called by CodexTaskManager
    // It needs to return a promise that resolves with the backend response
    // or rejects with an error.
    try {
      const response = await fetch('/api/codex/execute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prompt, options }),
      });
      const data = await response.json();
      if (!response.ok) {
        // If backend sends a JSON error, use it. Otherwise, a generic one.
        throw new Error(data.error || `Codex execution failed (status: ${response.status})`);
      }
      return Promise.resolve(data); // Resolve with the full response data
    } catch (err) {
      console.error("Codex execute error:", err);
      return Promise.reject(err); // Reject with the error
    }
  };

  return (
    <div className="App">
      <header className="App-header">
        <h1>Codex UI</h1>
      </header>
      <main className="App-main">
        {isLoading && <p className="loading-message">Loading initial configuration...</p>}
        {error && <p className="error-message">Initial Load Error: {error}</p>}
        
        <div className="layout-row"> {/* New row for side-by-side layout */}
          <div className="config-agents-column"> {/* Column for Config and Agents */}
            <div className="config-section">
              <ConfigManager
                currentApiKey={apiKey}
                currentProjectDirectory={projectDirectory}
                onApiKeyChange={handleApiKeyUpdate}
                onProjectDirChange={handleProjectDirUpdate}
              />
            </div>

            <div className="agents-section">
              <AgentsMdManager
                initialContent={agentsFileContent}
                onSave={handleSaveAgentsMd}
                filePath="AGENTS.md" // Fixed for now as per subtask
              />
            </div>
          </div>

          <div className="codex-task-section"> {/* New section for Codex Task Manager */}
            <CodexTaskManager onExecute={handleCodexExecute} />
          </div>
        </div>
        
      </main>
    </div>
  );
}

export default App;
