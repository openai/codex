'use client'

import { useRef, useMemo } from 'react'
import { Canvas, useFrame } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera, Html } from '@react-three/drei'
import * as THREE from 'three'

interface Commit {
  sha: string
  author: string
  message: string
  timestamp: number
  filesChanged: number
  insertions: number
  deletions: number
}

interface Scene3DProps {
  commits: Commit[]
  onCommitClick?: (commit: Commit) => void
  selectedCommitSha?: string
}

function CommitNode({ 
  commit, 
  position, 
  isSelected, 
  onClick 
}: { 
  commit: Commit
  position: [number, number, number]
  isSelected: boolean
  onClick: () => void
}) {
  const meshRef = useRef<THREE.Mesh>(null)
  
  // Pulse animation for selected commit
  useFrame((state) => {
    if (meshRef.current && isSelected) {
      meshRef.current.scale.setScalar(1 + Math.sin(state.clock.elapsedTime * 3) * 0.1)
    }
  })

  // Color based on change size
  const color = useMemo(() => {
    const total = commit.insertions + commit.deletions
    if (total > 1000) return '#ff6b6b' // Large change - red
    if (total > 100) return '#ffd93d' // Medium - yellow
    return '#6bcf7f' // Small - green
  }, [commit])

  // Size based on files changed
  const scale = useMemo(() => {
    return Math.min(0.5 + commit.filesChanged * 0.1, 2)
  }, [commit])

  return (
    <mesh
      ref={meshRef}
      position={position}
      onClick={onClick}
      scale={isSelected ? scale * 1.2 : scale}
    >
      <sphereGeometry args={[1, 32, 32]} />
      <meshStandardMaterial
        color={color}
        emissive={isSelected ? color : '#000000'}
        emissiveIntensity={isSelected ? 0.5 : 0}
        metalness={0.5}
        roughness={0.2}
      />
      <Html distanceFactor={10}>
        <div className="px-2 py-1 bg-black/80 text-white text-xs rounded whitespace-nowrap pointer-events-none">
          {commit.message.slice(0, 50)}
        </div>
      </Html>
    </mesh>
  )
}

function CommitEdges({ commits }: { commits: Commit[] }) {
  const points = useMemo(() => {
    const pts: THREE.Vector3[] = []
    commits.forEach((commit, i) => {
      if (i > 0) {
        const prevPos = calculatePosition(commits[i - 1], i - 1, commits.length)
        const currPos = calculatePosition(commit, i, commits.length)
        pts.push(new THREE.Vector3(...prevPos))
        pts.push(new THREE.Vector3(...currPos))
      }
    })
    return pts
  }, [commits])

  const lineGeometry = useMemo(() => {
    const geometry = new THREE.BufferGeometry().setFromPoints(points)
    return geometry
  }, [points])

  return (
    <lineSegments geometry={lineGeometry}>
      <lineBasicMaterial color="#667eea" opacity={0.3} transparent />
    </lineSegments>
  )
}

function calculatePosition(
  commit: Commit,
  index: number,
  total: number
): [number, number, number] {
  // Spiral layout
  const angle = (index / total) * Math.PI * 8
  const radius = 30 + (index / total) * 20
  const height = (index / total) * 50

  return [
    Math.cos(angle) * radius,
    height,
    Math.sin(angle) * radius
  ]
}

export function Scene3D({ commits, onCommitClick, selectedCommitSha }: Scene3DProps) {
  return (
    <div className="w-full h-full">
      <Canvas>
        <PerspectiveCamera makeDefault position={[50, 30, 50]} />
        <OrbitControls
          enableDamping
          dampingFactor={0.05}
          minDistance={10}
          maxDistance={200}
        />

        {/* Lighting */}
        <ambientLight intensity={0.4} />
        <pointLight position={[10, 10, 10]} intensity={1} />
        <pointLight position={[-10, -10, -10]} intensity={0.5} color="#764ba2" />
        
        {/* Commit nodes */}
        {commits.map((commit, i) => (
          <CommitNode
            key={commit.sha}
            commit={commit}
            position={calculatePosition(commit, i, commits.length)}
            isSelected={commit.sha === selectedCommitSha}
            onClick={() => onCommitClick?.(commit)}
          />
        ))}

        {/* Connection lines */}
        <CommitEdges commits={commits} />

        {/* Grid helper */}
        <gridHelper args={[100, 100, '#ffffff', '#333333']} />

        {/* Axes helper */}
        <axesHelper args={[50]} />
      </Canvas>
    </div>
  )
}

