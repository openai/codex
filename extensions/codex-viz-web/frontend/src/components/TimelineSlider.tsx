import { useState, useEffect, useCallback } from 'react'
import { useThree } from '@react-three/fiber'
import './TimelineSlider.css'

interface TimelineSliderProps {
  minDate: number
  maxDate: number
  currentDate: number
  onDateChange: (date: number) => void
  commits: Array<{ sha: string; timestamp: string }>
}

export default function TimelineSlider({
  minDate,
  maxDate,
  currentDate,
  onDateChange,
  commits,
}: TimelineSliderProps) {
  const { camera } = useThree()
  const [isPlaying, setIsPlaying] = useState(false)
  const [playbackSpeed, setPlaybackSpeed] = useState(1)
  const sliderRef = useState(() => document.createElement('input'))[0]

  // Calculate date range
  const dateRange = maxDate - minDate
  const progress = ((currentDate - minDate) / dateRange) * 100

  // Format date for display
  const formatDate = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toLocaleDateString('ja-JP', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    })
  }

  // Handle slider change
  const handleSliderChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const value = parseFloat(event.target.value)
      const newDate = minDate + (value / 100) * dateRange
      onDateChange(newDate)
    },
    [minDate, dateRange, onDateChange]
  )

  // Playback animation
  useEffect(() => {
    if (!isPlaying) return

    let animationFrame: number
    let lastTime = performance.now()

    const animate = (currentTime: number) => {
      const delta = (currentTime - lastTime) / 1000
      lastTime = currentTime

      // Calculate new date
      const increment = (delta * playbackSpeed * dateRange) / 60
      const newDate = Math.min(maxDate, currentDate + increment)

      if (newDate >= maxDate) {
        setIsPlaying(false)
      }

      onDateChange(newDate)
      animationFrame = requestAnimationFrame(animate)
    }

    animationFrame = requestAnimationFrame(animate)

    return () => cancelAnimationFrame(animationFrame)
  }, [isPlaying, currentDate, dateRange, maxDate, onDateChange, playbackSpeed])

  // Get commit count at current time
  const getCommitCount = () => {
    return commits.filter(
      (c) => new Date(c.timestamp).getTime() <= currentDate
    ).length
  }

  return (
    <div className="timeline-slider">
      <div className="timeline-header">
        <span className="timeline-label">📅 Timeline</span>
        <div className="timeline-stats">
          <span className="commit-count">
            {getCommitCount()} / {commits.length} commits
          </span>
        </div>
      </div>

      <div className="slider-container">
        <input
          type="range"
          min="0"
          max="100"
          step="0.1"
          value={progress}
          onChange={handleSliderChange}
          className="timeline-range"
        />

        {/* Timeline markers */}
        <div className="timeline-markers">
          {[...Array(5)].map((_, i) => {
            const markerProgress = (i / 4) * 100
            const markerDate = minDate + (markerProgress / 100) * dateRange

            return (
              <div
                key={i}
                className="timeline-marker"
                style={{ left: `${markerProgress}%` }}
              >
                <div className="marker-dot" />
                <span className="marker-label">{formatDate(markerDate)}</span>
              </div>
            )
          })}
        </div>
      </div>

      <div className="timeline-controls">
        {/* Play/Pause button */}
        <button
          className="control-button"
          onClick={() => setIsPlaying(!isPlaying)}
          aria-label={isPlaying ? 'Pause' : 'Play'}
        >
          {isPlaying ? '⏸' : '▶'}
        </button>

        {/* Speed control */}
        <div className="speed-control">
          <span className="speed-label">Speed:</span>
          <button
            className={`speed-button ${playbackSpeed === 0.5 ? 'active' : ''}`}
            onClick={() => setPlaybackSpeed(0.5)}
          >
            0.5×
          </button>
          <button
            className={`speed-button ${playbackSpeed === 1 ? 'active' : ''}`}
            onClick={() => setPlaybackSpeed(1)}
          >
            1×
          </button>
          <button
            className={`speed-button ${playbackSpeed === 2 ? 'active' : ''}`}
            onClick={() => setPlaybackSpeed(2)}
          >
            2×
          </button>
          <button
            className={`speed-button ${playbackSpeed === 4 ? 'active' : ''}`}
            onClick={() => setPlaybackSpeed(4)}
          >
            4×
          </button>
        </div>

        {/* Reset button */}
        <button
          className="control-button"
          onClick={() => onDateChange(minDate)}
          aria-label="Reset to beginning"
        >
          ⏮
        </button>

        {/* Jump to end */}
        <button
          className="control-button"
          onClick={() => onDateChange(maxDate)}
          aria-label="Jump to end"
        >
          ⏭
        </button>
      </div>

      <div className="timeline-footer">
        <span className="current-date">{formatDate(currentDate)}</span>
      </div>
    </div>
  )
}

