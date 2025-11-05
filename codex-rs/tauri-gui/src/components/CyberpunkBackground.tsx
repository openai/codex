// CyberpunkBackground.tsx - Neon Grid Background with Scanline Effect
import { useEffect, useRef } from 'react'

export const CyberpunkBackground = () => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    
    // Set canvas size
    const resizeCanvas = () => {
      canvas.width = window.innerWidth
      canvas.height = window.innerHeight
    }
    resizeCanvas()
    window.addEventListener('resize', resizeCanvas)
    
    // Draw animated grid
    let animationFrame: number
    let offset = 0
    
    const drawGrid = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      
      const gridSize = 50
      offset = (offset + 0.5) % gridSize
      
      // Draw vertical lines
      ctx.strokeStyle = 'rgba(0, 212, 255, 0.15)'
      ctx.lineWidth = 1
      
      for (let x = -offset; x < canvas.width + gridSize; x += gridSize) {
        ctx.beginPath()
        ctx.moveTo(x, 0)
        ctx.lineTo(x, canvas.height)
        ctx.stroke()
      }
      
      // Draw horizontal lines
      ctx.strokeStyle = 'rgba(184, 79, 255, 0.15)'
      for (let y = -offset; y < canvas.height + gridSize; y += gridSize) {
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(canvas.width, y)
        ctx.stroke()
      }
      
      // Draw glowing intersections
      ctx.fillStyle = 'rgba(0, 212, 255, 0.3)'
      for (let x = -offset; x < canvas.width + gridSize; x += gridSize * 2) {
        for (let y = -offset; y < canvas.height + gridSize; y += gridSize * 2) {
          ctx.beginPath()
          ctx.arc(x, y, 2, 0, Math.PI * 2)
          ctx.fill()
        }
      }
      
      animationFrame = requestAnimationFrame(drawGrid)
    }
    
    drawGrid()
    
    return () => {
      cancelAnimationFrame(animationFrame)
      window.removeEventListener('resize', resizeCanvas)
    }
  }, [])
  
  return (
    <>
      <div className="cyber-grid-bg" />
      <div className="cyber-scanline" />
      <canvas 
        ref={canvasRef}
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          width: '100%',
          height: '100%',
          zIndex: -2,
          opacity: 0.4,
          pointerEvents: 'none',
        }}
      />
    </>
  )
}


