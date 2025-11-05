// Scene3D.tsx - Desktop-only 3D Git Visualization (Cyberpunk Edition)
// Electric glow effects with neon colors - KAMUI 4D inspired

import { useRef, useMemo, useEffect } from 'react'
import { Canvas } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera } from '@react-three/drei'
import { EffectComposer, Bloom, ChromaticAberration } from '@react-three/postprocessing'
import * as THREE from 'three'
import { GPUOptimizer } from './GPUOptimizer'

// Cyberpunk color palette
const CYBERPUNK_COLORS = [
  '#00d4ff', // Electric Blue
  '#b84fff', // Neon Purple
  '#ff006e', // Hot Pink
  '#39ff14', // Acid Green
  '#ffff00', // Cyber Yellow
  '#ff3131', // Neon Red
  '#00ffff', // Cyan
  '#ff00ff', // Magenta
]

const getColorfulCommitColor = (index: number, isSelected: boolean): string => {
  if (isSelected) return '#ffffff' // White glow for selected
  return CYBERPUNK_COLORS[index % CYBERPUNK_COLORS.length]
}

export interface Commit3D {
  sha: string
  message: string
  author: string
  timestamp: string
  x: number
  y: number
  z: number
  color: string
  parents: string[]
}

interface Scene3DProps {
  commits: Commit3D[]
  onCommitClick?: (sha: string) => void
  selectedCommitSha?: string
}

function CommitNodes({ commits, onCommitClick, selectedCommitSha }: Scene3DProps) {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  
  // Instanced mesh for performance (100+ commits) - Colorful cyberpunk style
  const { matrices, colors } = useMemo(() => {
    const matrices: THREE.Matrix4[] = []
    const colors: number[] = []
    
    commits.forEach((commit, index) => {
      const matrix = new THREE.Matrix4()
      const position = new THREE.Vector3(commit.x, commit.y, commit.z)
      const isSelected = commit.sha === selectedCommitSha
      const scale = isSelected ? 1.8 : 1.0
      
      matrix.compose(
        position,
        new THREE.Quaternion(),
        new THREE.Vector3(scale, scale, scale)
      )
      matrices.push(matrix)
      
      // Use colorful cyberpunk palette instead of original color
      const cyberpunkColor = getColorfulCommitColor(index, isSelected)
      const color = new THREE.Color(cyberpunkColor)
      colors.push(color.r, color.g, color.b)
    })
    
    return { matrices, colors }
  }, [commits, selectedCommitSha])
  
  useEffect(() => {
    if (meshRef.current) {
      matrices.forEach((matrix, i) => {
        meshRef.current!.setMatrixAt(i, matrix)
        meshRef.current!.setColorAt(i, new THREE.Color(
          colors[i * 3],
          colors[i * 3 + 1],
          colors[i * 3 + 2]
        ))
      })
      meshRef.current.instanceMatrix.needsUpdate = true
      if (meshRef.current.instanceColor) {
        meshRef.current.instanceColor.needsUpdate = true
      }
    }
  }, [matrices, colors])
  
  return (
    <instancedMesh
      ref={meshRef}
      args={[undefined, undefined, commits.length]}
      onClick={(e) => {
        if (e.instanceId !== undefined && onCommitClick) {
          onCommitClick(commits[e.instanceId].sha)
        }
      }}
    >
      <sphereGeometry args={[0.5, 16, 16]} />
      <meshStandardMaterial vertexColors />
    </instancedMesh>
  )
}

function CommitEdges({ commits }: { commits: Commit3D[] }) {
  const lines = useMemo(() => {
    const lineObjects: { line: THREE.Line; color: string }[] = []
    
    commits.forEach((commit, index) => {
      commit.parents.forEach(parentSha => {
        const parent = commits.find(c => c.sha === parentSha)
        if (parent) {
          const points = [
            new THREE.Vector3(commit.x, commit.y, commit.z),
            new THREE.Vector3(parent.x, parent.y, parent.z)
          ]
          const geometry = new THREE.BufferGeometry().setFromPoints(points)
          
          // Colorful edges with additive blending for glow effect
          const edgeColor = CYBERPUNK_COLORS[index % CYBERPUNK_COLORS.length]
          const material = new THREE.LineBasicMaterial({ 
            color: new THREE.Color(edgeColor),
            transparent: true,
            opacity: 0.6,
            blending: THREE.AdditiveBlending, // Glow effect
          })
          const line = new THREE.Line(geometry, material)
          lineObjects.push({ line, color: edgeColor })
        }
      })
    })
    
    return lineObjects
  }, [commits])
  
  return (
    <>
      {lines.map(({ line }, i) => (
        <primitive key={i} object={line} />
      ))}
    </>
  )
}

export default function Scene3D({ commits, onCommitClick, selectedCommitSha }: Scene3DProps) {
  return (
    <Canvas>
      <GPUOptimizer commitCount={commits.length} maxFPS={60} />
      <PerspectiveCamera makeDefault position={[0, 0, 50]} />
      
      {/* Lighting for cyberpunk atmosphere */}
      <ambientLight intensity={0.3} />
      <directionalLight position={[10, 10, 5]} intensity={0.5} color="#00d4ff" />
      <pointLight position={[-10, -10, -5]} intensity={0.8} color="#b84fff" />
      <pointLight position={[0, 10, 0]} intensity={0.6} color="#ff006e" />
      
      <CommitNodes 
        commits={commits}
        onCommitClick={onCommitClick}
        selectedCommitSha={selectedCommitSha}
      />
      <CommitEdges commits={commits} />
      
      {/* Cyberpunk-styled grid */}
      <gridHelper 
        args={[100, 50, '#00d4ff', '#b84fff']} 
        position={[0, -10, 0]}
      />
      
      <OrbitControls enableDamping dampingFactor={0.05} />
      
      {/* Post-processing effects for glow */}
      <EffectComposer>
        <Bloom 
          luminanceThreshold={0.2} 
          luminanceSmoothing={0.9} 
          intensity={2.0}
          radius={0.8}
        />
        <ChromaticAberration offset={[0.002, 0.002]} />
      </EffectComposer>
    </Canvas>
  )
}

