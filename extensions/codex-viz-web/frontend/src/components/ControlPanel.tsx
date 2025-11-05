import React from 'react'
import './ControlPanel.css'

interface ControlPanelProps {
  viewMode: 'commits' | 'heatmap' | 'branches' | 'all'
  onViewModeChange: (mode: 'commits' | 'heatmap' | 'branches' | 'all') => void
  repoPath: string
  onRepoPathChange: (path: string) => void
  showStats: boolean
  onToggleStats: () => void
  onShowHelp: () => void
}

export default function ControlPanel({
  viewMode,
  onViewModeChange,
  repoPath,
  onRepoPathChange,
  showStats,
  onToggleStats,
  onShowHelp,
}: ControlPanelProps) {
  return (
    <div className="control-panel">
      <div className="panel-header">
        <h1>🎨 Codex Repository Visualizer</h1>
      </div>

      <div className="control-group">
        <label>Repository Path (optional)</label>
        <input
          type="text"
          value={repoPath}
          onChange={(e) => onRepoPathChange(e.target.value)}
          placeholder="Leave empty for current directory"
          className="path-input"
        />
      </div>

      <div className="control-group">
        <label>View Mode</label>
        <div className="button-group">
          <button
            className={viewMode === 'commits' ? 'active' : ''}
            onClick={() => onViewModeChange('commits')}
          >
            📊 Commits
          </button>
          <button
            className={viewMode === 'heatmap' ? 'active' : ''}
            onClick={() => onViewModeChange('heatmap')}
          >
            🔥 Heatmap
          </button>
          <button
            className={viewMode === 'branches' ? 'active' : ''}
            onClick={() => onViewModeChange('branches')}
          >
            🌿 Branches
          </button>
          <button
            className={viewMode === 'all' ? 'active' : ''}
            onClick={() => onViewModeChange('all')}
          >
            🌐 All
          </button>
        </div>
      </div>

      <div className="control-group">
        <label>
          <input
            type="checkbox"
            checked={showStats}
            onChange={onToggleStats}
          />
          Show Performance Stats
        </label>
      </div>

      <div className="panel-footer">
        <button 
          className="help-button"
          onClick={onShowHelp}
          aria-label="Show keyboard shortcuts"
        >
          ⌨️ Shortcuts (?)
        </button>
        <button 
          className="help-button"
          onClick={() => {
            // This will be handled in App.tsx
            const event = new CustomEvent('toggle-bookmarks')
            window.dispatchEvent(event)
          }}
          aria-label="Show bookmarks"
        >
          🔖 Bookmarks
        </button>
      </div>
    </div>
  )
}

