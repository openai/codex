// ARScene.tsx - AR Git Visualization
// ARCore/ARKit対応 - 空間にGitグラフを配置

import { useRef, useState, useEffect } from 'react'
import { Canvas } from '@react-three/fiber'
import { Text } from '@react-three/drei'
import { XRButton, useHitTest, useXR } from '@react-three/xr'
import * as THREE from 'three'
import { Commit3D } from '../git/Scene3D'
import { WebXRProvider } from '../vr/WebXRProvider'

interface ARSceneProps {
  commits: Commit3D[]
  onCommitClick?: (sha: string) => void
}

// AR Hitテスト用のリティクル（照準）
function ARReticle() {
  const reticleRef = useRef<THREE.Mesh>(null)
  
  useHitTest((hitMatrix) => {
    if (reticleRef.current) {
      reticleRef.current.visible = true
      reticleRef.current.matrix.copy(hitMatrix)
    }
  })
  
  return (
    <mesh ref={reticleRef} rotation-x={-Math.PI / 2}>
      <ringGeometry args={[0.1, 0.15, 32]} />
      <meshBasicMaterial color="#00d4ff" transparent opacity={0.7} />
    </mesh>
  )
}

// AR空間に配置されたGitグラフ
function ARGitGraph({ commits, onCommitClick }: ARSceneProps) {
  const [placed, setPlaced] = useState(false)
  const [anchorPosition, setAnchorPosition] = useState<THREE.Vector3>(new THREE.Vector3())
  const groupRef = useRef<THREE.Group>(null)
  
  // Hitテストで配置位置を決定
  useHitTest((hitMatrix) => {
    if (!placed) {
      const position = new THREE.Vector3()
      position.setFromMatrixPosition(hitMatrix)
      setAnchorPosition(position)
    }
  })
  
  useEffect(() => {
    if (groupRef.current && placed) {
      groupRef.current.position.copy(anchorPosition)
    }
  }, [anchorPosition, placed])
  
  // タップで配置
  const handleTap = () => {
    if (!placed) {
      setPlaced(true)
    }
  }
  
  return (
    <group ref={groupRef} onClick={handleTap}>
      {placed && (
        <>
          {/* Commits as colorful spheres */}
          {commits.map((commit, index) => {
            const color = CYBERPUNK_COLORS[index % CYBERPUNK_COLORS.length]
            
            return (
              <group key={commit.sha}>
                <mesh
                  position={[
                    commit.x * 0.05, // ARではスケールダウン
                    commit.y * 0.05,
                    commit.z * 0.05,
                  ]}
                  onClick={() => onCommitClick?.(commit.sha)}
                >
                  <sphereGeometry args={[0.02, 16, 16]} />
                  <meshStandardMaterial 
                    color={color}
                    emissive={color}
                    emissiveIntensity={0.5}
                  />
                </mesh>
                
                {/* Commit message label */}
                <Text
                  position={[
                    commit.x * 0.05,
                    commit.y * 0.05 + 0.04,
                    commit.z * 0.05,
                  ]}
                  fontSize={0.01}
                  color="#ffffff"
                  anchorX="center"
                  anchorY="bottom"
                >
                  {commit.message.slice(0, 20)}
                </Text>
              </group>
            )
          })}
          
          {/* Edges */}
          {commits.map((commit, commitIndex) =>
            commit.parents.map((parentSha, parentIndex) => {
              const parent = commits.find(c => c.sha === parentSha)
              if (!parent) return null
              
              const points = [
                new THREE.Vector3(commit.x * 0.05, commit.y * 0.05, commit.z * 0.05),
                new THREE.Vector3(parent.x * 0.05, parent.y * 0.05, parent.z * 0.05),
              ]
              
              const color = CYBERPUNK_COLORS[commitIndex % CYBERPUNK_COLORS.length]
              
              return (
                <line key={`${commit.sha}-${parentIndex}`}>
                  <bufferGeometry>
                    <bufferAttribute
                      attach="attributes-position"
                      count={points.length}
                      array={new Float32Array(points.flatMap(p => [p.x, p.y, p.z]))}
                      itemSize={3}
                    />
                  </bufferGeometry>
                  <lineBasicMaterial 
                    color={color}
                    transparent
                    opacity={0.6}
                    blending={THREE.AdditiveBlending}
                  />
                </line>
              )
            })
          )}
        </>
      )}
      
      {!placed && (
        <Text
          position={[0, 0.5, -1]}
          fontSize={0.05}
          color="#00d4ff"
          anchorX="center"
        >
          Tap to place Git graph
        </Text>
      )}
    </group>
  )
}

function ARSceneContent({ commits, onCommitClick }: ARSceneProps) {
  return (
    <>
      <ambientLight intensity={1.0} />
      <ARReticle />
      <ARGitGraph commits={commits} onCommitClick={onCommitClick} />
    </>
  )
}

export default function ARScene({ commits, onCommitClick }: ARSceneProps) {
  return (
    <div style={{ width: '100%', height: '100vh', position: 'relative' }}>
      {/* AR Entry Button */}
      <XRButton 
        mode="AR"
        style={{
          position: 'absolute',
          top: '20px',
          right: '20px',
          zIndex: 100,
          padding: '12px 24px',
          background: 'linear-gradient(135deg, #39ff14, #00d4ff)',
          border: 'none',
          borderRadius: '6px',
          color: '#0a0a0f',
          fontFamily: 'monospace',
          fontWeight: '700',
          fontSize: '16px',
          cursor: 'pointer',
          boxShadow: '0 0 20px rgba(57, 255, 20, 0.6)',
        }}
      />
      
      <Canvas>
        <WebXRProvider>
          <ARSceneContent commits={commits} onCommitClick={onCommitClick} />
        </WebXRProvider>
      </Canvas>
    </div>
  )
}

