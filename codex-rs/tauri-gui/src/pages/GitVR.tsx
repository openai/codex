// GitVR.tsx - 4D Git Visualization Page
// Kamui4d-style Git history exploration with time-travel

import { useState, useEffect, useMemo } from "react"
import Scene4D from "../components/git/Scene4D"
import "../styles/GitVR.css"

interface Commit3D {
  sha: string
  message: string
  author: string
  timestamp: string
  x: number
  y: number
  z: number
  color: string
  parents: string[]
}

export default function GitVR() {
  const [commits, setCommits] = useState<Commit3D[]>([])
  const [selectedCommit, setSelectedCommit] = useState<Commit3D | null>(null)

  useEffect(() => {
    // TODO: Load from Codex backend via Tauri IPC
    loadMockCommits()
  }, [])

  const loadMockCommits = () => {
    // Generate sample commit graph
    const mockCommits: Commit3D[] = []
    for (let i = 0; i < 50; i++) {
      mockCommits.push({
        sha: `commit-${i}`,
        message: `Commit ${i}: Implement feature #${i}`,
        author: "Developer",
        timestamp: new Date(Date.now() - i * 86400000).toISOString(),
        x: Math.sin(i * 0.5) * 20,
        y: i * 2,
        z: Math.cos(i * 0.5) * 20,
        color: `hsl(${i * 7}, 70%, 60%)`,
        parents: i > 0 ? [`commit-${i - 1}`] : []
      })
    }
    setCommits(mockCommits)
  }

  const handleCommitClick = (sha: string) => {
    const commit = commits.find(c => c.sha === sha)
    setSelectedCommit(commit || null)
  }

  // Convert to 4D (add w-axis based on timestamp)
  const commits4D = useMemo(() => {
    if (commits.length === 0) return []
    
    const timestamps = commits.map(c => new Date(c.timestamp).getTime())
    const minTime = Math.min(...timestamps)
    const maxTime = Math.max(...timestamps)
    const timeRange = maxTime - minTime || 1
    
    return commits.map(c => ({
      ...c,
      w: (new Date(c.timestamp).getTime() - minTime) / timeRange
    }))
  }, [commits])

  return (
    <div className="git-vr-page">
      <div className="info-panel">
        <h2>4D Git Visualization (Kamui4d-style)</h2>
        <p><strong>Total Commits:</strong> {commits.length}</p>
        <div style={{ marginTop: '10px', padding: '10px', background: '#f0f0f0', borderRadius: '5px' }}>
          <p style={{ margin: 0, fontSize: '14px', color: '#666' }}>
            Use the time slider to travel through commit history
          </p>
        </div>
        {selectedCommit && (
          <div className="commit-details" style={{ marginTop: '20px', padding: '15px', background: '#fff', borderRadius: '5px', border: '1px solid #ddd' }}>
            <h3 style={{ marginTop: 0 }}>Selected Commit</h3>
            <p><strong>SHA:</strong> {selectedCommit.sha}</p>
            <p><strong>Message:</strong> {selectedCommit.message}</p>
            <p><strong>Author:</strong> {selectedCommit.author}</p>
            <p><strong>Date:</strong> {new Date(selectedCommit.timestamp).toLocaleString()}</p>
          </div>
        )}
      </div>

      <Scene4D
        commits={commits4D}
        onCommitClick={handleCommitClick}
        selectedCommitSha={selectedCommit?.sha}
      />
    </div>
  )
}
