import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import '../styles/Orchestration.css'

type AgentType = 'codex' | 'geminicli' | 'claudecode'

interface AgentTask {
  id: string
  agent: AgentType
  prompt: string
  worktree_path?: string
  timeout_seconds?: number
}

interface ResourceCapacity {
  max_concurrent: number
  active_tasks: number
  available_slots: number
}

interface SystemStats {
  cpu_usage_percent: number
  memory_used_bytes: number
  memory_total_bytes: number
  memory_usage_percent: number
  active_agents: number
  cpu_cores: number
}

interface AgentResult {
  agent: string
  success: boolean
  output: string
  elapsed_seconds: number
  error?: string
}

interface AgentProgress {
  agent: string
  status: 'pending' | 'running' | 'completed' | 'failed' | 'timeout'
  progress_percent: number
  current_step?: string
}

interface ComparisonResult {
  total_agents: number
  successful: number
  failed: number
  fastest_agent?: string
  fastest_time?: number
}

export default function Orchestration() {
  const [tasks, setTasks] = useState<AgentTask[]>([
    { id: '1', agent: 'codex', prompt: '' },
    { id: '2', agent: 'geminicli', prompt: '' },
    { id: '3', agent: 'claudecode', prompt: '' },
  ])
  const [isRunning, setIsRunning] = useState(false)
  const [progress, setProgress] = useState<AgentProgress[]>([])
  const [results, setResults] = useState<AgentResult[]>([])
  const [comparison, setComparison] = useState<ComparisonResult | null>(null)
  const [capacity, setCapacity] = useState<ResourceCapacity | null>(null)
  const [systemStats, setSystemStats] = useState<SystemStats | null>(null)

  // Load resource capacity and system stats on mount
  useEffect(() => {
    const loadStats = async () => {
      try {
        const cap = await invoke<ResourceCapacity>('get_resource_capacity')
        setCapacity(cap)
        
        const stats = await invoke<SystemStats>('get_system_stats')
        setSystemStats(stats)
      } catch (err) {
        console.error('Failed to load stats:', err)
      }
    }
    
    loadStats()
    const statsInterval = setInterval(loadStats, 2000) // Update every 2s
    
    return () => clearInterval(statsInterval)
  }, [])

  useEffect(() => {
    let interval: number | null = null

    if (isRunning) {
      interval = window.setInterval(async () => {
        try {
          const prog = await invoke<AgentProgress[]>('get_orchestration_progress')
          setProgress(prog)

          // Check if all completed
          const allDone = prog.every(p => 
            p.status === 'completed' || p.status === 'failed' || p.status === 'timeout'
          )
          if (allDone && prog.length > 0) {
            setIsRunning(false)
          }
        } catch (err) {
          console.error('Failed to get progress:', err)
        }
      }, 500)
    }

    return () => {
      if (interval) clearInterval(interval)
    }
  }, [isRunning])

  const handlePromptChange = (id: string, prompt: string) => {
    setTasks(tasks.map(task => 
      task.id === id ? { ...task, prompt } : task
    ))
  }

  const addAgent = (agentType: AgentType) => {
    const newId = Date.now().toString()
    setTasks([...tasks, { id: newId, agent: agentType, prompt: '' }])
  }

  const removeAgent = (id: string) => {
    if (tasks.length <= 1) {
      alert('At least one agent is required')
      return
    }
    setTasks(tasks.filter(task => task.id !== id))
  }

  const changeAgentType = (id: string, newType: AgentType) => {
    setTasks(tasks.map(task =>
      task.id === id ? { ...task, agent: newType } : task
    ))
  }

  const handleExecute = async () => {
    // Validate tasks
    const validTasks = tasks.filter(t => t.prompt.trim().length > 0)
    if (validTasks.length === 0) {
      alert('Please enter at least one prompt')
      return
    }

    setIsRunning(true)
    setProgress([])
    setResults([])
    setComparison(null)

    try {
      const agentResults = await invoke<AgentResult[]>('orchestrate_parallel', { tasks: validTasks })
      setResults(agentResults)

      // Get comparison
      const comp = await invoke<ComparisonResult>('compare_agent_results', { results: agentResults })
      setComparison(comp)
    } catch (err) {
      console.error('Orchestration failed:', err)
      alert(`Error: ${err}`)
    } finally {
      setIsRunning(false)
    }
  }

  const getAgentIcon = (agent: string) => {
    switch (agent.toLowerCase()) {
      case 'codex': return '🤖'
      case 'geminicli': return '✨'
      case 'claudecode': return '🧠'
      default: return '🔧'
    }
  }

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'pending': return '#888'
      case 'running': return '#3b82f6'
      case 'completed': return '#10b981'
      case 'failed': return '#ef4444'
      case 'timeout': return '#f59e0b'
      default: return '#888'
    }
  }

  return (
    <div className="orchestration-page">
      <div className="orchestration-header">
        <h1>🎭 AI Orchestration</h1>
        <p>Dynamic parallel execution with unlimited agents and resource management</p>
        
        {capacity && systemStats && (
          <div className="resource-info">
            <div className="resource-stat">
              <span className="stat-label">CPU Cores</span>
              <span className="stat-value">{systemStats.cpu_cores}</span>
            </div>
            <div className="resource-stat">
              <span className="stat-label">Max Concurrent</span>
              <span className="stat-value">{capacity.max_concurrent}</span>
            </div>
            <div className="resource-stat">
              <span className="stat-label">Active / Available</span>
              <span className="stat-value">{capacity.active_tasks} / {capacity.available_slots}</span>
            </div>
            <div className="resource-stat">
              <span className="stat-label">CPU Usage</span>
              <span className="stat-value">{systemStats.cpu_usage_percent.toFixed(1)}%</span>
            </div>
            <div className="resource-stat">
              <span className="stat-label">Memory Usage</span>
              <span className="stat-value">{systemStats.memory_usage_percent.toFixed(1)}%</span>
            </div>
          </div>
        )}
      </div>

      <div className="task-setup">
        <div className="task-header-row">
          <h2>Task Configuration ({tasks.length} agents)</h2>
          <div className="add-agent-controls">
            <button 
              className="add-agent-button"
              onClick={() => addAgent('codex')}
              disabled={isRunning}
              title="Add Codex agent"
            >
              🤖 Add Codex
            </button>
            <button 
              className="add-agent-button"
              onClick={() => addAgent('geminicli')}
              disabled={isRunning}
              title="Add GeminiCLI agent"
            >
              ✨ Add Gemini
            </button>
            <button 
              className="add-agent-button"
              onClick={() => addAgent('claudecode')}
              disabled={isRunning}
              title="Add Claudecode agent"
            >
              🧠 Add Claude
            </button>
          </div>
        </div>

        {tasks.map((task) => (
          <div key={task.id} className="task-card">
            <div className="task-header">
              <select
                className="agent-selector"
                value={task.agent}
                onChange={(e) => changeAgentType(task.id, e.target.value as AgentType)}
                disabled={isRunning}
              >
                <option value="codex">🤖 Codex</option>
                <option value="geminicli">✨ GeminiCLI</option>
                <option value="claudecode">🧠 Claudecode</option>
              </select>
              <button
                className="remove-agent-button"
                onClick={() => removeAgent(task.id)}
                disabled={isRunning || tasks.length <= 1}
                title="Remove agent"
              >
                ❌
              </button>
            </div>
            <textarea
              className="task-prompt"
              placeholder={`Enter task for ${task.agent}...`}
              value={task.prompt}
              onChange={(e) => handlePromptChange(task.id, e.target.value)}
              disabled={isRunning}
              rows={3}
            />
          </div>
        ))}

        <button 
          className="execute-button"
          onClick={handleExecute}
          disabled={isRunning}
        >
          {isRunning ? '⏳ Running...' : `🚀 Execute ${tasks.length} Agent${tasks.length > 1 ? 's' : ''} in Parallel`}
        </button>
      </div>

      {progress.length > 0 && (
        <div className="progress-section">
          <h2>Execution Progress</h2>
          <div className="progress-grid">
            {progress.map((prog) => (
              <div key={prog.agent} className="progress-card">
                <div className="progress-header">
                  <span>{getAgentIcon(prog.agent)} {prog.agent.toUpperCase()}</span>
                  <span 
                    className="progress-status"
                    style={{ color: getStatusColor(prog.status) }}
                  >
                    {prog.status.toUpperCase()}
                  </span>
                </div>
                <div className="progress-bar-container">
                  <div 
                    className="progress-bar-fill"
                    style={{ 
                      width: `${prog.progress_percent}%`,
                      backgroundColor: getStatusColor(prog.status)
                    }}
                  />
                </div>
                <div className="progress-text">
                  {prog.current_step || `${Math.round(prog.progress_percent)}%`}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {results.length > 0 && (
        <div className="results-section">
          <h2>Results</h2>
          
          {comparison && (
            <div className="comparison-card">
              <h3>📊 Competition Summary</h3>
              <div className="comparison-stats">
                <div className="stat">
                  <span className="stat-label">Total Agents</span>
                  <span className="stat-value">{comparison.total_agents}</span>
                </div>
                <div className="stat">
                  <span className="stat-label">Successful</span>
                  <span className="stat-value success">{comparison.successful}</span>
                </div>
                <div className="stat">
                  <span className="stat-label">Failed</span>
                  <span className="stat-value failed">{comparison.failed}</span>
                </div>
                {comparison.fastest_agent && (
                  <div className="stat winner">
                    <span className="stat-label">🏆 Winner</span>
                    <span className="stat-value">
                      {getAgentIcon(comparison.fastest_agent)} {comparison.fastest_agent.toUpperCase()}
                      <small>({comparison.fastest_time?.toFixed(2)}s)</small>
                    </span>
                  </div>
                )}
              </div>
            </div>
          )}

          <div className="results-grid">
            {results.map((result, index) => (
              <div 
                key={index} 
                className={`result-card ${result.success ? 'success' : 'failed'}`}
              >
                <div className="result-header">
                  <span>{getAgentIcon(result.agent)} {result.agent.toUpperCase()}</span>
                  <span className="result-time">{result.elapsed_seconds.toFixed(2)}s</span>
                </div>
                <div className="result-content">
                  {result.success ? (
                    <pre className="result-output">{result.output}</pre>
                  ) : (
                    <div className="result-error">
                      ❌ Error: {result.error || 'Unknown error'}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

