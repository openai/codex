// SceneVR.tsx - VR-optimized Git Visualization
// Full WebXR support with hand tracking and teleportation

import { useRef, useMemo, useEffect, useState } from 'react'
import { Canvas } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera, Text } from '@react-three/drei'
import { EffectComposer, Bloom, ChromaticAberration } from '@react-three/postprocessing'
import { Controllers, Hands, XRButton, useXR } from '@react-three/xr'
import * as THREE from 'three'
import { Commit3D } from './Scene3D'
import { WebXRProvider, xrStore } from '../vr/WebXRProvider'

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
  if (isSelected) return '#ffffff'
  return CYBERPUNK_COLORS[index % CYBERPUNK_COLORS.length]
}

interface SceneVRProps {
  commits: Commit3D[]
  onCommitClick?: (sha: string) => void
  selectedCommitSha?: string
}

function VRCommitNodes({ commits, onCommitClick, selectedCommitSha }: SceneVRProps) {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  const { isPresenting } = useXR()
  
  const { matrices, colors } = useMemo(() => {
    const matrices: THREE.Matrix4[] = []
    const colors: number[] = []
    
    commits.forEach((commit, index) => {
      const matrix = new THREE.Matrix4()
      const position = new THREE.Vector3(commit.x, commit.y, commit.z)
      const isSelected = commit.sha === selectedCommitSha
      
      // VRモードでは大きめに表示
      const scale = isPresenting ? (isSelected ? 2.5 : 1.5) : (isSelected ? 1.8 : 1.0)
      
      matrix.compose(
        position,
        new THREE.Quaternion(),
        new THREE.Vector3(scale, scale, scale)
      )
      matrices.push(matrix)
      
      const cyberpunkColor = getColorfulCommitColor(index, isSelected)
      const color = new THREE.Color(cyberpunkColor)
      colors.push(color.r, color.g, color.b)
    })
    
    return { matrices, colors }
  }, [commits, selectedCommitSha, isPresenting])
  
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
      <sphereGeometry args={[0.5, 32, 32]} />
      <meshStandardMaterial 
        vertexColors 
        emissive="#ffffff"
        emissiveIntensity={0.2}
      />
    </instancedMesh>
  )
}

function VRCommitEdges({ commits }: { commits: Commit3D[] }) {
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
          
          const edgeColor = CYBERPUNK_COLORS[index % CYBERPUNK_COLORS.length]
          const material = new THREE.LineBasicMaterial({ 
            color: new THREE.Color(edgeColor),
            transparent: true,
            opacity: 0.7,
            blending: THREE.AdditiveBlending,
            linewidth: 2, // VRで見やすく
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

// VR用情報パネル（3D空間に配置）
function VRInfoPanel({ commit }: { commit: Commit3D | null }) {
  if (!commit) return null
  
  return (
    <group position={[0, 5, -10]}>
      <Text
        fontSize={0.5}
        color="#00d4ff"
        anchorX="center"
        anchorY="middle"
      >
        {commit.message.slice(0, 50)}
      </Text>
      <Text
        fontSize={0.3}
        color="#b84fff"
        anchorX="center"
        anchorY="middle"
        position={[0, -0.7, 0]}
      >
        {commit.author}
      </Text>
      <Text
        fontSize={0.25}
        color="#e0e0e0"
        anchorX="center"
        anchorY="middle"
        position={[0, -1.2, 0]}
      >
        SHA: {commit.sha.slice(0, 8)}
      </Text>
    </group>
  )
}

function VRScene({ commits, onCommitClick, selectedCommitSha }: SceneVRProps) {
  const [selectedCommit, setSelectedCommit] = useState<Commit3D | null>(null)
  
  useEffect(() => {
    const commit = commits.find(c => c.sha === selectedCommitSha) ?? null
    setSelectedCommit(commit)
  }, [selectedCommitSha, commits])
  
  return (
    <>
      {/* Lighting for VR */}
      <ambientLight intensity={0.4} />
      <directionalLight position={[10, 10, 5]} intensity={0.6} color="#00d4ff" />
      <pointLight position={[-10, -10, -5]} intensity={1.0} color="#b84fff" />
      <pointLight position={[0, 10, 0]} intensity={0.8} color="#ff006e" />
      
      {/* Git Graph */}
      <VRCommitNodes 
        commits={commits}
        onCommitClick={onCommitClick}
        selectedCommitSha={selectedCommitSha}
      />
      <VRCommitEdges commits={commits} />
      
      {/* VR Info Panel */}
      <VRInfoPanel commit={selectedCommit} />
      
      {/* Cyberpunk Grid */}
      <gridHelper 
        args={[100, 50, '#00d4ff', '#b84fff']} 
        position={[0, -10, 0]}
      />
      
      {/* VR Controllers */}
      <Controllers />
      
      {/* Hand Tracking */}
      <Hands />
      
      {/* Post-processing effects */}
      <EffectComposer>
        <Bloom 
          luminanceThreshold={0.2} 
          luminanceSmoothing={0.9} 
          intensity={1.5} // VRでは少し抑える
          radius={0.8}
        />
        <ChromaticAberration offset={[0.001, 0.001]} />
      </EffectComposer>
    </>
  )
}

export default function SceneVR({ commits, onCommitClick, selectedCommitSha }: SceneVRProps) {
  return (
    <div style={{ width: '100%', height: '100vh', position: 'relative' }}>
      {/* VR Entry Button */}
      <XRButton 
        mode="VR"
        style={{
          position: 'absolute',
          top: '20px',
          right: '20px',
          zIndex: 100,
          padding: '12px 24px',
          background: 'linear-gradient(135deg, #00d4ff, #b84fff)',
          border: 'none',
          borderRadius: '6px',
          color: '#fff',
          fontFamily: 'monospace',
          fontWeight: '700',
          fontSize: '16px',
          cursor: 'pointer',
          boxShadow: '0 0 20px rgba(0, 212, 255, 0.6)',
        }}
      />
      
      <Canvas>
        <WebXRProvider>
          <PerspectiveCamera makeDefault position={[0, 0, 50]} />
          <OrbitControls enableDamping dampingFactor={0.05} />
          
          <VRScene 
            commits={commits}
            onCommitClick={onCommitClick}
            selectedCommitSha={selectedCommitSha}
          />
        </WebXRProvider>
      </Canvas>
    </div>
  )
}

