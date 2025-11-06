'use client'

import { useState, useEffect } from 'react'
import dynamic from 'next/dynamic'
import { useGitWorker } from '@/lib/hooks/useGitWorker'

// Dynamically import VR component (client-side only)
const Scene3DVXR = dynamic(
  () => import('@/components/visualizations/Scene3DVXR'),
  { ssr: false }
)

export default function GitVRPage() {
  const [commits, setCommits] = useState<any[]>([])
  const [selectedCommit, setSelectedCommit] = useState<any | null>(null)
  const [loading, setLoading] = useState(true)
  const { parseCommits } = useGitWorker()

  useEffect(() => {
    loadGitData()
  }, [])

  const loadGitData = async () => {
    try {
      // Load git data (replace with actual data source)
      const response = await fetch('/api/git/commits')
      const data = await response.json()
      
      const parsed = await parseCommits(data)
      setCommits(parsed.commits)
      setLoading(false)
    } catch (error) {
      console.error('Failed to load git data:', error)
      setLoading(false)
    }
  }

  const handleCommitClick = (commit: any) => {
    setSelectedCommit(commit)
    console.log('Selected commit:', commit)
  }

  if (loading) {
    return (
      <div style={{ 
        display: 'flex', 
        alignItems: 'center', 
        justifyContent: 'center', 
        height: '100vh',
        backgroundColor: '#000',
        color: '#fff',
      }}>
        <div>
          <h1>Loading Git VR...</h1>
          <p>Preparing immersive visualization</p>
        </div>
      </div>
    )
  }

  return (
    <div style={{ width: '100%', height: '100vh', backgroundColor: '#000' }}>
      <div style={{
        position: 'absolute',
        top: 20,
        left: 20,
        color: '#fff',
        zIndex: 1000,
        backgroundColor: 'rgba(0,0,0,0.7)',
        padding: '10px 20px',
        borderRadius: '8px',
      }}>
        <h2>Codex Git VR</h2>
        <p>Put on your VR headset and click "Enter VR"</p>
        <p>Commits: {commits.length}</p>
        {selectedCommit && (
          <div style={{ marginTop: '10px', fontSize: '12px' }}>
            <strong>Selected:</strong> {selectedCommit.sha.substring(0, 8)}
            <br />
            {selectedCommit.message}
          </div>
        )}
      </div>

      <Scene3DVXR
        commits={commits}
        onCommitClick={handleCommitClick}
        selectedCommitSha={selectedCommit?.sha}
      />
    </div>
  )
}

