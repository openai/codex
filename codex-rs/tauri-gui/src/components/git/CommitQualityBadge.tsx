import { useMemo } from 'react'

interface CommitQualityBadgeProps {
  score: number
  size?: 'small' | 'medium' | 'large'
  showLabel?: boolean
}

export function CommitQualityBadge({ 
  score, 
  size = 'medium',
  showLabel = true 
}: CommitQualityBadgeProps) {
  const { color, label } = useMemo(() => {
    if (score >= 80) return { color: '#00ff00', label: 'High' }
    if (score >= 60) return { color: '#ffff00', label: 'Medium' }
    if (score >= 40) return { color: '#ff8800', label: 'Low' }
    return { color: '#ff0000', label: 'Critical' }
  }, [score])

  const sizeMap = {
    small: { width: 40, height: 20, fontSize: 10 },
    medium: { width: 60, height: 24, fontSize: 12 },
    large: { width: 80, height: 32, fontSize: 14 }
  }

  const dimensions = sizeMap[size]

  return (
    <div
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: dimensions.width,
        height: dimensions.height,
        backgroundColor: color,
        color: '#000',
        borderRadius: 4,
        fontSize: dimensions.fontSize,
        fontWeight: 'bold',
        padding: '2px 6px',
        boxShadow: '0 2px 4px rgba(0,0,0,0.2)',
      }}
    >
      {showLabel ? label : Math.round(score)}
    </div>
  )
}

interface QualityScoreRingProps {
  score: number
  size?: number
}

export function QualityScoreRing({ score, size = 50 }: QualityScoreRingProps) {
  const color = useMemo(() => {
    if (score >= 80) return '#00ff00'
    if (score >= 60) return '#ffff00'
    if (score >= 40) return '#ff8800'
    return '#ff0000'
  }, [score])

  const circumference = 2 * Math.PI * (size / 2 - 5)
  const strokeDashoffset = circumference - (score / 100) * circumference

  return (
    <svg width={size} height={size}>
      <circle
        cx={size / 2}
        cy={size / 2}
        r={size / 2 - 5}
        fill="transparent"
        stroke="#333"
        strokeWidth="3"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={size / 2 - 5}
        fill="transparent"
        stroke={color}
        strokeWidth="3"
        strokeDasharray={circumference}
        strokeDashoffset={strokeDashoffset}
        transform={`rotate(-90 ${size / 2} ${size / 2})`}
      />
      <text
        x="50%"
        y="50%"
        textAnchor="middle"
        dy=".3em"
        fontSize={size / 3}
        fill="#fff"
        fontWeight="bold"
      >
        {Math.round(score)}
      </text>
    </svg>
  )
}


