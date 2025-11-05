import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { CommitQualityBadge, QualityScoreRing } from './CommitQualityBadge'

interface CommitQualityScore {
  sha: string
  code_quality: number
  test_coverage: number
  documentation: number
  complexity: number
  overall: number
  insights: string[]
  issues: QualityIssue[]
}

interface QualityIssue {
  severity: 'critical' | 'high' | 'medium' | 'low'
  category: string
  description: string
  file_path?: string
  line_number?: number
}

interface QualityInsightsProps {
  commitSha: string
  repoPath: string
}

export function QualityInsights({ commitSha, repoPath }: QualityInsightsProps) {
  const [quality, setQuality] = useState<CommitQualityScore | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadQuality()
  }, [commitSha, repoPath])

  const loadQuality = async () => {
    try {
      setLoading(true)
      setError(null)
      
      const result = await invoke<CommitQualityScore>('analyze_commit_quality', {
        repoPath,
        commitSha
      })
      
      setQuality(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  if (loading) {
    return (
      <div className="quality-insights loading">
        <div className="spinner">Analyzing commit quality...</div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="quality-insights error">
        <p>Failed to analyze: {error}</p>
      </div>
    )
  }

  if (!quality) {
    return null
  }

  return (
    <div className="quality-insights">
      <div className="quality-header">
        <h3>AI Quality Analysis</h3>
        <CommitQualityBadge score={quality.overall} size="large" />
      </div>

      <div className="quality-metrics">
        <MetricCard
          label="Code Quality"
          score={quality.code_quality}
          icon="📝"
        />
        <MetricCard
          label="Test Coverage"
          score={quality.test_coverage}
          icon="🧪"
        />
        <MetricCard
          label="Documentation"
          score={quality.documentation}
          icon="📚"
        />
        <MetricCard
          label="Complexity"
          score={100 - quality.complexity}
          icon="🔧"
          inverted
        />
      </div>

      {quality.insights.length > 0 && (
        <div className="quality-insights-list">
          <h4>✨ Insights</h4>
          <ul>
            {quality.insights.map((insight, i) => (
              <li key={i}>{insight}</li>
            ))}
          </ul>
        </div>
      )}

      {quality.issues.length > 0 && (
        <div className="quality-issues">
          <h4>⚠️ Issues</h4>
          {quality.issues.map((issue, i) => (
            <IssueCard key={i} issue={issue} />
          ))}
        </div>
      )}
    </div>
  )
}

function MetricCard({ 
  label, 
  score, 
  icon, 
  inverted = false 
}: { 
  label: string
  score: number
  icon: string
  inverted?: boolean
}) {
  return (
    <div className="metric-card">
      <div className="metric-icon">{icon}</div>
      <div className="metric-content">
        <div className="metric-label">{label}</div>
        <QualityScoreRing score={inverted ? 100 - score : score} size={60} />
      </div>
    </div>
  )
}

function IssueCard({ issue }: { issue: QualityIssue }) {
  const severityColors = {
    critical: '#ff0000',
    high: '#ff8800',
    medium: '#ffff00',
    low: '#00ff00'
  }

  return (
    <div 
      className="issue-card"
      style={{ borderLeftColor: severityColors[issue.severity] }}
    >
      <div className="issue-header">
        <span className="issue-severity">{issue.severity.toUpperCase()}</span>
        <span className="issue-category">{issue.category}</span>
      </div>
      <p className="issue-description">{issue.description}</p>
      {issue.file_path && (
        <div className="issue-location">
          📁 {issue.file_path}
          {issue.line_number && `:${issue.line_number}`}
        </div>
      )}
    </div>
  )
}


