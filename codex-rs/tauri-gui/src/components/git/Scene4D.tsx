// Scene4D.tsx - 4D Git Visualization with Time-Travel Axis
// Extends Scene3D with W-dimension (time axis) for Kamui4d-style visualization

import { useState, useMemo, useEffect } from 'react'
import Scene3D, { Commit3D } from './Scene3D'
import * as THREE from 'three'
import { invoke } from '@tauri-apps/api/core'

export interface Commit4D extends Commit3D {
  w: number  // Time dimension (0.0 = oldest, 1.0 = newest)
  qualityScore?: number  // AI quality score (0-100)
}

interface CommitQualityScore {
  sha: string
  overall: number
}

interface Scene4DProps {
  commits: Commit4D[]
  onCommitClick?: (sha: string) => void
  selectedCommitSha?: string
}

function adjustColorBrightness(hexColor: string, factor: number): string {
  const color = new THREE.Color(hexColor)
  color.multiplyScalar(0.5 + factor * 0.5)
  return '#' + color.getHexString()
}

// Get quality-based color
function getQualityColor(score?: number): string {
  if (!score) return '#888888'  // Gray for unanalyzed
  if (score >= 80) return '#00ff00'  // Green: High quality
  if (score >= 60) return '#ffff00'  // Yellow: Medium quality
  if (score >= 40) return '#ff8800'  // Orange: Needs improvement
  return '#ff0000'  // Red: Low quality
}

export default function Scene4D({ commits, onCommitClick, selectedCommitSha }: Scene4DProps) {
  const [timePosition, setTimePosition] = useState(1.0)  // Start at newest
  const [qualityScores, setQualityScores] = useState<Map<string, number>>(new Map())
  
  // Load quality scores for visible commits
  useEffect(() => {
    const loadQualityScores = async () => {
      try {
        // Only analyze commits that don't have scores yet
        const unanalyzedCommits = commits.filter(c => !qualityScores.has(c.sha))
        
        if (unanalyzedCommits.length > 0) {
          // Batch analyze (limit to 10 at a time for performance)
          const batch = unanalyzedCommits.slice(0, 10)
          const shas = batch.map(c => c.sha)
          
          const scores = await invoke<CommitQualityScore[]>('analyze_commits_batch', {
            repoPath: '.', // TODO: Get from app state
            commitShas: shas
          })
          
          setQualityScores(prev => {
            const newMap = new Map(prev)
            scores.forEach(score => {
              newMap.set(score.sha, score.overall)
            })
            return newMap
          })
        }
      } catch (err) {
        console.error('Failed to load quality scores:', err)
      }
    }
    
    loadQualityScores()
  }, [commits])
  
  // Filter commits by time window and apply quality colors
  const visibleCommits = useMemo(() => {
    const timeWindow = 0.3  // Show ±30% of timeline
    return commits.filter(commit => {
      const timeDiff = Math.abs(commit.w - timePosition)
      return timeDiff <= timeWindow
    }).map(commit => {
      // Map 4D to 3D by adjusting opacity/size based on time distance
      const timeFactor = 1.0 - (Math.abs(commit.w - timePosition) / timeWindow)
      
      // Use quality score for color, fallback to original color
      const qualityScore = qualityScores.get(commit.sha) ?? commit.qualityScore
      const baseColor = qualityScore !== undefined ? getQualityColor(qualityScore) : commit.color
      
      return {
        ...commit,
        y: commit.y + (commit.w - timePosition) * 20,  // Y-axis shows time
        color: adjustColorBrightness(baseColor, timeFactor),
        qualityScore
      }
    })
  }, [commits, timePosition, qualityScores])
  
  return (
    <div style={{ width: '100%', height: '100vh' }}>
      <Scene3D
        commits={visibleCommits}
        onCommitClick={onCommitClick}
        selectedCommitSha={selectedCommitSha}
      />
      
      {/* Time Travel Control */}
      <div style={{
        position: 'absolute',
        bottom: 20,
        left: '50%',
        transform: 'translateX(-50%)',
        background: 'rgba(0,0,0,0.7)',
        padding: '20px',
        borderRadius: '10px',
        color: 'white',
        zIndex: 1000
      }}>
        <div style={{ marginBottom: '10px', textAlign: 'center' }}>
          <strong>Time Position:</strong> {(timePosition * 100).toFixed(0)}%
        </div>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={timePosition}
          onChange={(e) => setTimePosition(parseFloat(e.target.value))}
          style={{ width: '400px', cursor: 'pointer' }}
        />
        <div style={{ display: 'flex', gap: '10px', marginTop: '10px', justifyContent: 'center' }}>
          <button 
            onClick={() => setTimePosition(0)}
            style={{
              padding: '8px 16px',
              background: '#444',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer'
            }}
          >
            ← Oldest
          </button>
          <button 
            onClick={() => setTimePosition(0.5)}
            style={{
              padding: '8px 16px',
              background: '#444',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer'
            }}
          >
            Middle
          </button>
          <button 
            onClick={() => setTimePosition(1)}
            style={{
              padding: '8px 16px',
              background: '#444',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer'
            }}
          >
            Newest →
          </button>
        </div>
        <div style={{ marginTop: '10px', fontSize: '12px', textAlign: 'center', color: '#aaa' }}>
          Showing {visibleCommits.length} of {commits.length} commits
        </div>
      </div>
    </div>
  )
}

