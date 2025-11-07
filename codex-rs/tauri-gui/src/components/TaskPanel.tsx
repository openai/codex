// TaskPanel.tsx - Kamui4D風タスク管理パネル
// エージェント実行状態、リアルタイム更新、ドラッグ&ドロップ

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../styles/TaskPanel.css';

export interface Task {
  id: string;
  title: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  progress: number;
  agentType?: string;
  startTime?: string;
  endTime?: string;
  result?: string;
}

export interface TaskPanelProps {
  onTaskSelect?: (task: Task) => void;
}

export default function TaskPanel({ onTaskSelect }: TaskPanelProps) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedTask, setSelectedTask] = useState<string | null>(null);

  useEffect(() => {
    // TODO: Tauri events経由でリアルタイム更新
    loadMockTasks();
  }, []);

  const loadMockTasks = () => {
    const mockTasks: Task[] = [
      {
        id: '1',
        title: 'Code Review - main.rs',
        status: 'completed',
        progress: 100,
        agentType: 'CodeReviewer',
      },
      {
        id: '2',
        title: 'CUDA Analysis',
        status: 'running',
        progress: 65,
        agentType: 'PerformanceAnalyzer',
      },
      {
        id: '3',
        title: 'Generate Tests',
        status: 'pending',
        progress: 0,
        agentType: 'TestGenerator',
      },
    ];
    setTasks(mockTasks);
  };

  const handleTaskClick = (task: Task) => {
    setSelectedTask(task.id);
    if (onTaskSelect) {
      onTaskSelect(task);
    }
  };

  return (
    <div className="task-panel">
      <div className="task-panel-header">
        <h2>Tasks (Kamui4D-style)</h2>
        <button className="btn-add-task">+ New Task</button>
      </div>

      <div className="task-list">
        {tasks.map((task) => (
          <div
            key={task.id}
            className={`task-item ${task.status} ${selectedTask === task.id ? 'selected' : ''}`}
            onClick={() => handleTaskClick(task)}
          >
            <div className="task-status-indicator" />
            <div className="task-content">
              <h3>{task.title}</h3>
              {task.agentType && <span className="task-agent">{task.agentType}</span>}
              {task.status === 'running' && (
                <div className="task-progress">
                  <div className="progress-bar" style={{ width: `${task.progress}%` }} />
                  <span className="progress-text">{task.progress}%</span>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

























