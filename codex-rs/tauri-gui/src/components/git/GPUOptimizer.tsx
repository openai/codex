// GPUOptimizer.tsx - GPU rendering optimization for large commit graphs
// Automatically adjusts LOD and rendering settings based on commit count

import { useEffect, useRef } from 'react'
import { useThree } from '@react-three/fiber'
import * as THREE from 'three'

interface GPUOptimizerProps {
  commitCount: number
  maxFPS?: number
  enableShadows?: boolean
}

export function GPUOptimizer({ 
  commitCount, 
  maxFPS = 60,
  enableShadows = false 
}: GPUOptimizerProps) {
  const { gl, scene } = useThree()
  const lastTimeRef = useRef(0)
  
  useEffect(() => {
    // WebGL optimization settings
    gl.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    gl.shadowMap.enabled = enableShadows
    
    // Power preference hint (set during canvas initialization)
    // WebGL context is already created by React Three Fiber
    // We just configure the renderer here
    
    console.log('[GPU Optimizer] Initialized', {
      commits: commitCount,
      maxFPS,
      shadows: enableShadows,
      pixelRatio: gl.getPixelRatio()
    })
  }, [gl, commitCount, maxFPS, enableShadows])
  
  useEffect(() => {
    // Auto LOD for large commit graphs
    if (commitCount > 100) {
      console.log('[GPU Optimizer] Applying LOD optimizations')
      
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh) {
          // Simplify geometry for large datasets
          const mesh = object as THREE.Mesh
          
          if (commitCount > 500) {
            // Very aggressive LOD
            if (mesh.geometry instanceof THREE.SphereGeometry) {
              // Reduce sphere segments
              const newGeometry = new THREE.SphereGeometry(
                0.5,
                8,  // widthSegments (reduced from 16)
                8   // heightSegments (reduced from 16)
              )
              mesh.geometry.dispose()
              mesh.geometry = newGeometry
            }
          } else if (commitCount > 200) {
            // Moderate LOD
            if (mesh.geometry instanceof THREE.SphereGeometry) {
              const newGeometry = new THREE.SphereGeometry(
                0.5,
                12,  // widthSegments
                12   // heightSegments
              )
              mesh.geometry.dispose()
              mesh.geometry = newGeometry
            }
          }
          
          // Disable frustum culling for instanced meshes (they handle it internally)
          if (mesh instanceof THREE.InstancedMesh) {
            mesh.frustumCulled = true
          }
        }
      })
    }
  }, [commitCount, scene])
  
  useEffect(() => {
    // FPS limiter
    const animate = (time: number) => {
      const delta = time - lastTimeRef.current
      
      if (delta >= 1000 / maxFPS) {
        lastTimeRef.current = time
        
        // Performance monitoring
        if (delta > 1000 / 30) {
          console.warn('[GPU Optimizer] Low FPS detected:', {
            fps: Math.round(1000 / delta),
            commits: commitCount
          })
        }
      }
      
      requestAnimationFrame(animate)
    }
    
    const animationId = requestAnimationFrame(animate)
    
    return () => {
      cancelAnimationFrame(animationId)
    }
  }, [maxFPS, commitCount])
  
  return null
}

// Performance stats component
export function GPUPerformanceStats({ show = false }: { show?: boolean }) {
  const { gl } = useThree()
  
  useEffect(() => {
    if (!show) return
    
    const interval = setInterval(() => {
      const info = gl.info
      console.log('[GPU Stats]', {
        geometries: info.memory.geometries,
        textures: info.memory.textures,
        programs: info.programs?.length || 0,
        calls: info.render.calls,
        triangles: info.render.triangles,
        points: info.render.points
      })
    }, 5000)
    
    return () => clearInterval(interval)
  }, [gl, show])
  
  return null
}

