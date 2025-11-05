'use client'

import { useState, useEffect, useRef } from 'react'

interface TimelineProps {
  commits: Array<{ sha: string; timestamp: number; message: string; author: string }>
  onSeek: (index: number) => void
  currentIndex: number
}

export function Timeline({ commits, onSeek, currentIndex }: TimelineProps) {
  const [isPlaying, setIsPlaying] = useState(false)
  const [playbackSpeed, setPlaybackSpeed] = useState(1) // 1x, 2x, 4x, 8x
  const [loop, setLoop] = useState(false)
  const timelineRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!isPlaying) return

    const intervalMs = 100 / playbackSpeed // Base 100ms, faster with higher speed

    const interval = setInterval(() => {
      const nextIndex = currentIndex + 1
      
      if (nextIndex >= commits.length) {
        if (loop) {
          onSeek(0) // Loop back to start
        } else {
          setIsPlaying(false)
        }
      } else {
        onSeek(nextIndex)
      }
    }, intervalMs)

    return () => clearInterval(interval)
  }, [isPlaying, currentIndex, commits.length, playbackSpeed, loop, onSeek])

  const handlePlayPause = () => {
    if (currentIndex >= commits.length - 1) {
      onSeek(0) // Reset to start
    }
    setIsPlaying(!isPlaying)
  }

  const currentCommit = commits[currentIndex]
  const progress = (currentIndex / Math.max(commits.length - 1, 1)) * 100

  return (
    <div className="w-full p-4 bg-gray-800/50 backdrop-blur-lg rounded-xl border border-gray-700">
      <div className="flex items-center gap-4 mb-4">
        {/* Play/Pause */}
        <button
          onClick={handlePlayPause}
          className="p-3 bg-purple-500 hover:bg-purple-600 rounded-lg transition"
        >
          {isPlaying ? (
            <svg className="w-5 h-5 text-white" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clipRule="evenodd" />
            </svg>
          ) : (
            <svg className="w-5 h-5 text-white" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clipRule="evenodd" />
            </svg>
          )}
        </button>

        {/* Speed control */}
        <div className="flex items-center gap-2">
          <span className="text-sm text-gray-400">Speed:</span>
          <select
            value={playbackSpeed}
            onChange={(e) => setPlaybackSpeed(Number(e.target.value))}
            className="px-2 py-1 bg-gray-700 border border-gray-600 rounded text-white text-sm"
          >
            <option value={500}>0.5x</option>
            <option value={200}>1x</option>
            <option value={100}>2x</option>
            <option value={50}>4x</option>
          </select>
        </div>

        {/* Current commit info */}
        {currentCommit && (
          <div className="flex-1 min-w-0">
            <div className="text-sm font-mono text-gray-400 truncate">
              {currentCommit.sha.slice(0, 7)}
            </div>
            <div className="text-xs text-gray-500 truncate">
              {currentCommit.message}
            </div>
          </div>
        )}

        {/* Commit counter */}
        <div className="text-sm text-gray-400">
          {currentIndex + 1} / {commits.length}
        </div>
      </div>

      {/* Progress bar */}
      <div className="relative">
        <div className="h-2 bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-purple-500 to-pink-500 transition-all duration-200"
            style={{ width: `${progress}%` }}
          />
        </div>

        {/* Slider */}
        <input
          type="range"
          min={0}
          max={commits.length - 1}
          value={currentIndex}
          onChange={(e) => {
            setIsPlaying(false)
            onSeek(Number(e.target.value))
          }}
          className="absolute top-0 w-full h-2 opacity-0 cursor-pointer"
        />
      </div>

      {/* Date range */}
      {commits.length > 0 && (
        <div className="flex justify-between mt-2 text-xs text-gray-500">
          <span>{new Date(commits[0].timestamp).toLocaleDateString()}</span>
          <span>{new Date(commits[commits.length - 1].timestamp).toLocaleDateString()}</span>
        </div>
      )}
    </div>
  )
}

