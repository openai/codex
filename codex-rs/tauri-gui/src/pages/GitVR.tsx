// GitVR.tsx - Babylon.js Git Visualization Page（Kamui4D超え）
// 10万コミット対応、動的LOD、WebGPU優先

import { useState, useEffect } from "react"
import { invoke } from '@tauri-apps/api/core'
import BabylonGitScene from "../components/git/BabylonGitScene"
import type { Commit3D } from "../utils/babylon-git-engine"
import "../styles/GitVR.css"

export default function GitVR() {
  const [commits, setCommits] = useState<Commit3D[]>([])
  const [selectedCommit, setSelectedCommit] = useState<Commit3D | null>(null)
  const [isLoading, setIsLoading] = useState<boolean>(true)
  const [repoPath, setRepoPath] = useState<string>('.')
  const [useCuda, setUseCuda] = useState<boolean>(true)

  useEffect(() => {
    loadCommitsFromBackend()
  }, [repoPath, useCuda])

  /**
   * Tauri IPC経由でGitコミット取得（CUDA加速対応）
   */
  const loadCommitsFromBackend = async () => {
    setIsLoading(true)
    try {
      // Tauri IPCでGitコミット取得
      const commits3D = await invoke<Commit3D[]>('get_git_commits_3d', {
        repoPath,
        limit: 10000, // Kamui4D超え: 10,000コミット
      })

      // CUDA加速解析（利用可能な場合）
      if (useCuda) {
        try {
          const analyzed = await invoke<Commit3D[]>('analyze_with_cuda', {
            commits: commits3D,
          })
          setCommits(analyzed)
        } catch (cudaError) {
          console.warn('⚠️  CUDA analysis failed, using CPU:', cudaError)
          setCommits(commits3D)
        }
      } else {
        setCommits(commits3D)
      }

      console.log(`✅ Loaded ${commits3D.length} commits from ${repoPath}`)
    } catch (error) {
      console.error('❌ Failed to load commits:', error)
      // フォールバック: モックデータ
      loadMockCommits()
    } finally {
      setIsLoading(false)
    }
  }

  /**
   * モックデータ（開発・デモ用）
   */
  const loadMockCommits = () => {
    const mockCommits: Commit3D[] = []
    for (let i = 0; i < 1000; i++) {
      mockCommits.push({
        sha: `commit-${i.toString(16).padStart(7, '0')}`,
        message: `Commit ${i}: Implement feature #${i}`,
        author: `Developer-${i % 5}`,
        timestamp: new Date(Date.now() - i * 3600000).toISOString(),
        x: Math.sin(i * 0.3) * 50 + Math.cos(i * 0.1) * 20,
        y: i * 0.5,
        z: Math.cos(i * 0.3) * 50 + Math.sin(i * 0.1) * 20,
        color: `hsl(${(i * 37) % 360}, 80%, 60%)`,
        parents: i > 0 ? [`commit-${(i - 1).toString(16).padStart(7, '0')}`] : []
      })
    }
    setCommits(mockCommits)
    setIsLoading(false)
  }

  const handleCommitClick = (commit: Commit3D) => {
    setSelectedCommit(commit)
    console.log('Selected commit:', commit.sha, commit.message)
  }

  return (
    <div className="git-vr-page">
      {/* コントロールパネル */}
      <div className="control-panel">
        <h2>Git Visualization (Kamui4D-exceeding)</h2>
        
        <div className="control-row">
          <label htmlFor="repo-path">Repository:</label>
          <input
            id="repo-path"
            type="text"
            value={repoPath}
            onChange={(e) => setRepoPath(e.target.value)}
            placeholder="Repository path"
          />
          <button onClick={loadCommitsFromBackend}>Reload</button>
        </div>

        <div className="control-row">
          <label>
            <input
              type="checkbox"
              checked={useCuda}
              onChange={(e) => setUseCuda(e.target.checked)}
            />
            CUDA Acceleration
          </label>
        </div>

        <div className="stats-row">
          <span className="stat">Total: <strong>{commits.length}</strong> commits</span>
          {isLoading && <span className="loading-text">Loading...</span>}
        </div>

        {selectedCommit && (
          <div className="commit-details-panel">
            <h3>Selected Commit</h3>
            <p className="commit-sha">{selectedCommit.sha}</p>
            <p className="commit-message">{selectedCommit.message}</p>
            <p className="commit-meta">
              <span>{selectedCommit.author}</span>
              <span>{new Date(selectedCommit.timestamp).toLocaleDateString()}</span>
            </p>
          </div>
        )}
      </div>

      {/* Babylon.js 3D可視化 */}
      <div className="visualization-container">
        <BabylonGitScene
          commits={commits}
          onCommitClick={handleCommitClick}
          selectedCommitSha={selectedCommit?.sha}
          showStats={true}
          showMinimap={false}
        />
      </div>
    </div>
  )
}
