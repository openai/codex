// Scene3D.tsx - Desktop-only 3D Git Visualization
// No VR dependencies, clean Three.js implementation

import { useRef, useMemo, useEffect } from 'react'
import { Canvas } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera } from '@react-three/drei'
import * as THREE from 'three'
import { GPUOptimizer } from './GPUOptimizer'

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
  
  // Instanced mesh for performance (100+ commits)
  const { matrices, colors } = useMemo(() => {
    const matrices: THREE.Matrix4[] = []
    const colors: number[] = []
    
    commits.forEach((commit) => {
      const matrix = new THREE.Matrix4()
      const position = new THREE.Vector3(commit.x, commit.y, commit.z)
      const scale = commit.sha === selectedCommitSha ? 1.5 : 1.0
      
      matrix.compose(
        position,
        new THREE.Quaternion(),
        new THREE.Vector3(scale, scale, scale)
      )
      matrices.push(matrix)
      
      const color = new THREE.Color(commit.color)
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
    const lineObjects: THREE.Line[] = []
    
    commits.forEach((commit) => {
      commit.parents.forEach(parentSha => {
        const parent = commits.find(c => c.sha === parentSha)
        if (parent) {
          const points = [
            new THREE.Vector3(commit.x, commit.y, commit.z),
            new THREE.Vector3(parent.x, parent.y, parent.z)
          ]
          const geometry = new THREE.BufferGeometry().setFromPoints(points)
          const material = new THREE.LineBasicMaterial({ color: 0x666666 })
          const line = new THREE.Line(geometry, material)
          lineObjects.push(line)
        }
      })
    })
    
    return lineObjects
  }, [commits])
  
  return (
    <>
      {lines.map((line, i) => (
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
      <ambientLight intensity={0.6} />
      <directionalLight position={[10, 10, 5]} intensity={0.8} />
      <pointLight position={[-10, -10, -5]} intensity={0.5} />
      
      <CommitNodes 
        commits={commits}
        onCommitClick={onCommitClick}
        selectedCommitSha={selectedCommitSha}
      />
      <CommitEdges commits={commits} />
      
      <gridHelper args={[100, 50]} position={[0, -10, 0]} />
      <OrbitControls enableDamping dampingFactor={0.05} />
    </Canvas>
  )
}

