'use client'

import { useState, useEffect } from 'react'
import { Scene3D } from '../../../components/visualizations/Scene3DInstanced'
import { Timeline } from '../../../components/visualizations/Timeline'
import { getCommits, getBranches, getHeatmap, type Commit3D } from '../../../lib/api/git'

export default function VisualizationPage() {
  const [repoPath, setRepoPath] = useState('.')
  const [commits, setCommits] = useState<Commit3D[]>([])
  const [selectedCommitSha, setSelectedCommitSha] = useState<string | undefined>()
  const [currentIndex, setCurrentIndex] = useState(0)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [viewMode, setViewMode] = useState<'commits' | 'heatmap' | 'branches' | 'all'>('commits')

  useEffect(() => {
    loadData()
  }, [repoPath])

  const loadData = async () => {
    setLoading(true)
    setError(null)
    try {
      const commitData = await getCommits(repoPath, 1000)
      setCommits(commitData)
      if (commitData.length > 0) {
        setCurrentIndex(0)
        setSelectedCommitSha(commitData[0].sha)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load Git data')
    } finally {
      setLoading(false)
    }
  }

  const handleCommitClick = (commit: any) => {
    const index = commits.findIndex((c) => c.sha === commit.sha)
    if (index !== -1) {
      setCurrentIndex(index)
      setSelectedCommitSha(commit.sha)
    }
  }

  const handleSeek = (index: number) => {
    setCurrentIndex(index)
    if (commits[index]) {
      setSelectedCommitSha(commits[index].sha)
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-4xl font-bold text-white mb-2">Git Visualization</h1>
            <p className="text-gray-400">Kamui4d-style 3D/4D repository visualization</p>
          </div>

          <div className="flex items-center gap-4">
            {/* View Mode Selector */}
            <select
              value={viewMode}
              onChange={(e) => setViewMode(e.target.value as any)}
              className="px-4 py-2 bg-gray-800 text-white rounded-lg border border-gray-700 focus:outline-none focus:ring-2 focus:ring-purple-500"
            >
              <option value="commits">📊 Commits</option>
              <option value="heatmap">🔥 Heatmap</option>
              <option value="branches">🌿 Branches</option>
              <option value="all">🌐 All</option>
            </select>

            {/* Repo Path Input */}
            <input
              type="text"
              value={repoPath}
              onChange={(e) => setRepoPath(e.target.value)}
              placeholder="Repository path (. for current)"
              className="px-4 py-2 bg-gray-800 text-white rounded-lg border border-gray-700 focus:outline-none focus:ring-2 focus:ring-purple-500 w-64"
            />

            <button
              onClick={loadData}
              className="px-6 py-2 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-semibold transition"
            >
              🔄 Reload
            </button>
          </div>
        </div>

        {/* Error Display */}
        {error && (
          <div className="bg-red-500/20 border border-red-500 text-red-200 px-4 py-3 rounded-lg mb-6">
            ❌ {error}
          </div>
        )}

        {/* Loading State */}
        {loading ? (
          <div className="text-center text-gray-400 py-20">
            <div className="animate-spin text-6xl mb-4">⏳</div>
            <p>Analyzing repository...</p>
          </div>
        ) : commits.length === 0 ? (
          <div className="text-center text-gray-400 py-20">
            <div className="text-6xl mb-4">📂</div>
            <p className="text-xl">No Git repository found</p>
            <p className="text-sm mt-2">Make sure you're in a Git repository directory</p>
          </div>
        ) : (
          <div className="space-y-6">
            {/* Stats Bar */}
            <div className="grid grid-cols-4 gap-4">
              <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-1">Total Commits</div>
                <div className="text-3xl font-bold text-white">{commits.length}</div>
              </div>
              <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-1">Unique Authors</div>
                <div className="text-3xl font-bold text-white">
                  {new Set(commits.map((c) => c.author)).size}
                </div>
              </div>
              <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-1">Branches</div>
                <div className="text-3xl font-bold text-white">
                  {new Set(commits.map((c) => c.branch)).size}
                </div>
              </div>
              <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-lg p-4">
                <div className="text-sm text-gray-400 mb-1">Current</div>
                <div className="text-3xl font-bold text-white">
                  {currentIndex + 1} / {commits.length}
                </div>
              </div>
            </div>

            {/* 3D Visualization */}
            <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl overflow-hidden">
              <div className="p-4 border-b border-gray-700">
                <h2 className="text-xl font-bold text-white">3D Commit Graph</h2>
                <p className="text-sm text-gray-400">
                  Drag to rotate • Scroll to zoom • Click commits to select
                </p>
              </div>
              <div className="h-[600px]">
                <Scene3D
                  commits={commits}
                  onCommitClick={handleCommitClick}
                  selectedCommitSha={selectedCommitSha}
                />
              </div>
            </div>

            {/* Timeline Control */}
            <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl overflow-hidden">
              <Timeline
                commits={commits.map((c) => ({
                  sha: c.sha,
                  timestamp: new Date(c.timestamp).getTime(),
                  message: c.message,
                  author: c.author,
                }))}
                onSeek={handleSeek}
                currentIndex={currentIndex}
              />
            </div>

            {/* Commit Details */}
            {selectedCommitSha && (
              <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
                <h2 className="text-xl font-bold text-white mb-4">Commit Details</h2>
                {(() => {
                  const commit = commits.find((c) => c.sha === selectedCommitSha)
                  if (!commit) return null

                  return (
                    <div className="space-y-4">
                      <div className="grid grid-cols-2 gap-4">
                        <div>
                          <div className="text-sm text-gray-400 mb-1">SHA</div>
                          <div className="text-white font-mono text-sm">
                            {commit.sha.substring(0, 12)}
                          </div>
                        </div>
                        <div>
                          <div className="text-sm text-gray-400 mb-1">Branch</div>
                          <div className="text-white">{commit.branch}</div>
                        </div>
                        <div>
                          <div className="text-sm text-gray-400 mb-1">Author</div>
                          <div className="text-white">{commit.author}</div>
                        </div>
                        <div>
                          <div className="text-sm text-gray-400 mb-1">Date</div>
                          <div className="text-white">
                            {new Date(commit.timestamp).toLocaleString()}
                          </div>
                        </div>
                      </div>

                      <div>
                        <div className="text-sm text-gray-400 mb-1">Message</div>
                        <div className="text-white bg-gray-700/50 p-3 rounded">
                          {commit.message}
                        </div>
                      </div>

                      {commit.parents.length > 0 && (
                        <div>
                          <div className="text-sm text-gray-400 mb-1">Parents</div>
                          <div className="flex gap-2 flex-wrap">
                            {commit.parents.map((parent) => (
                              <span
                                key={parent}
                                className="text-xs bg-gray-700 px-2 py-1 rounded font-mono"
                              >
                                {parent.substring(0, 8)}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      <div>
                        <div className="text-sm text-gray-400 mb-1">3D Coordinates</div>
                        <div className="text-sm text-gray-300 font-mono bg-gray-700/50 p-3 rounded">
                          X: {commit.x.toFixed(2)} | Y: {commit.y.toFixed(2)} | Z:{' '}
                          {commit.z.toFixed(2)}
                        </div>
                      </div>
                    </div>
                  )
                })()}
              </div>
            )}

            {/* Author Legend */}
            <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
              <h2 className="text-xl font-bold text-white mb-4">Authors</h2>
              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
                {Array.from(new Set(commits.map((c) => c.author_email))).map((email) => {
                  const commit = commits.find((c) => c.author_email === email)
                  if (!commit) return null

                  return (
                    <div
                      key={email}
                      className="flex items-center gap-2 bg-gray-700/50 p-2 rounded"
                    >
                      <div
                        className="w-4 h-4 rounded-full"
                        style={{ backgroundColor: commit.color }}
                      />
                      <div className="text-sm text-white truncate">{commit.author}</div>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

