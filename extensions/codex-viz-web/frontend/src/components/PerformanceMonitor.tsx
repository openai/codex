import { useEffect, useState } from 'react'
import { useFrame, useThree } from '@react-three/fiber'
import './PerformanceMonitor.css'

interface PerformanceMetrics {
  fps: number
  memory: number
  drawCalls: number
  triangles: number
  gpuTime: number
}

export default function PerformanceMonitor() {
  const { gl } = useThree()
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    fps: 0,
    memory: 0,
    drawCalls: 0,
    triangles: 0,
    gpuTime: 0,
  })

  let frameCount = 0
  let lastTime = performance.now()

  useFrame(() => {
    frameCount++
    const currentTime = performance.now()
    const elapsed = currentTime - lastTime

    if (elapsed >= 1000) {
      const fps = Math.round((frameCount * 1000) / elapsed)
      const info = gl.info

      setMetrics({
        fps,
        memory: (performance as any).memory
          ? Math.round((performance as any).memory.usedJSHeapSize / 1048576)
          : 0,
        drawCalls: info.render.calls,
        triangles: info.render.triangles,
        gpuTime: 0, // WebGL doesn't expose GPU time directly
      })

      frameCount = 0
      lastTime = currentTime
    }
  })

  const getFpsColor = (fps: number) => {
    if (fps >= 55) return '#22c55e'
    if (fps >= 30) return '#eab308'
    return '#ef4444'
  }

  return (
    <div className="performance-monitor">
      <div className="perf-header">
        <span className="perf-title">⚡ Performance</span>
      </div>
      
      <div className="perf-metrics">
        <div className="perf-metric">
          <span className="perf-label">FPS</span>
          <span 
            className="perf-value perf-value-large"
            style={{ color: getFpsColor(metrics.fps) }}
          >
            {metrics.fps}
          </span>
        </div>

        <div className="perf-metric">
          <span className="perf-label">Memory</span>
          <span className="perf-value">{metrics.memory} MB</span>
        </div>

        <div className="perf-metric">
          <span className="perf-label">Draw Calls</span>
          <span className="perf-value">{metrics.drawCalls}</span>
        </div>

        <div className="perf-metric">
          <span className="perf-label">Triangles</span>
          <span className="perf-value">
            {(metrics.triangles / 1000).toFixed(1)}K
          </span>
        </div>
      </div>

      <div className="perf-bar">
        <div 
          className="perf-bar-fill"
          style={{ 
            width: `${Math.min(100, (metrics.fps / 60) * 100)}%`,
            background: getFpsColor(metrics.fps),
          }}
        />
      </div>
    </div>
  )
}

