'use client'

import { useState } from 'react'
import { useXR } from '@react-three/xr'
import { Text } from '@react-three/drei'
import * as THREE from 'three'

interface VRInterfaceProps {
  // (no properties declared here; properties declared after End of Selection)
}
  commits: any[]
  selectedCommit?: any
  onTimelineChange?: (time: number) => void
}

/**
 * VR UI Panel - Floating interface in VR space
 */
export function VRPanel({ position, children }: { 
  position: [number, number, number]
  children: React.ReactNode 
}) {
  return (
    <group position={position}>
      {/* Panel background */}
      <mesh>
        <planeGeometry args={[2, 1.5]} />
        <meshStandardMaterial 
          color="#1a1a1a" 
          opacity={0.9} 
          transparent 
          side={THREE.DoubleSide}
        />
      </mesh>
      
      {/* Panel border */}
      <mesh position={[0, 0, 0.001]}>
        <planeGeometry args={[2.05, 1.55]} />
        <meshBasicMaterial 
          color="#0070f3" 
          opacity={0.3} 
          transparent 
          side={THREE.DoubleSide}
        />
      </mesh>
      
      {children}
    </group>
  )
}

/**
 * Main VR Interface Component
 */
export default function VRInterface({
  commits,
  selectedCommit,
  onTimelineChange,
}: VRInterfaceProps) {
  const { isPresenting } = useXR()
  const [timelineValue, setTimelineValue] = useState(0)

  if (!isPresenting) return null

  return (
    <>
      {/* Stats Panel */}
      <VRPanel position={[-3, 1.6, -2]}>
        <Text
          position={[0, 0.6, 0.01]}
          fontSize={0.15}
          color="#ffffff"
          anchorX="center"
          anchorY="middle"
        >
          Codex Git VR
        </Text>
        
        <Text
          position={[0, 0.3, 0.01]}
          fontSize={0.08}
          color="#aaaaaa"
          anchorX="center"
          anchorY="middle"
        >
          Total Commits: {commits.length}
        </Text>
        
        <Text
          position={[0, 0.1, 0.01]}
          fontSize={0.08}
          color="#aaaaaa"
          anchorX="center"
          anchorY="middle"
        >
          Mode: VR Immersive
        </Text>
      </VRPanel>

      {/* Selected Commit Details */}
      {selectedCommit && (
        <VRPanel position={[3, 1.6, -2]}>
          <Text
            position={[0, 0.6, 0.01]}
            fontSize={0.12}
            color="#0070f3"
            anchorX="center"
            anchorY="middle"
          >
            Selected Commit
          </Text>
          
          <Text
            position={[0, 0.3, 0.01]}
            fontSize={0.07}
            color="#ffffff"
            anchorX="center"
            anchorY="middle"
            maxWidth={1.8}
          >
            {selectedCommit.message}
          </Text>
          
          <Text
            position={[0, 0, 0.01]}
            fontSize={0.06}
            color="#888888"
            anchorX="center"
            anchorY="middle"
          >
            {selectedCommit.author}
          </Text>
          
          <Text
            position={[0, -0.2, 0.01]}
            fontSize={0.05}
            color="#666666"
            anchorX="center"
            anchorY="middle"
          >
            {selectedCommit.sha.substring(0, 8)}
          </Text>
        </VRPanel>
      )}

      {/* Timeline Control Panel */}
      <VRPanel position={[0, 0.5, -2]}>
        <Text
          position={[0, 0.6, 0.01]}
          fontSize={0.1}
          color="#ffffff"
          anchorX="center"
          anchorY="middle"
        >
          Timeline
        </Text>
        
        {/* Timeline slider (simplified representation) */}
        <mesh position={[-0.8, 0.2, 0.01]}>
          <boxGeometry args={[1.6, 0.05, 0.02]} />
          <meshStandardMaterial color="#333333" />
        </mesh>
        
        {/* Timeline position indicator */}
        <mesh position={[
          -0.8 + (timelineValue * 1.6),
          0.2,
          0.02
        ]}>
          <sphereGeometry args={[0.08, 16, 16]} />
          <meshStandardMaterial color="#0070f3" emissive="#0070f3" emissiveIntensity={0.5} />
        </mesh>
        
        <Text
          position={[0, -0.1, 0.01]}
          fontSize={0.06}
          color="#aaaaaa"
          anchorX="center"
          anchorY="middle"
        >
          {Math.floor(timelineValue * 100)}%
        </Text>
      </VRPanel>

      {/* Hand Menu (attached to left hand) */}
      <HandMenu />
    </>
  )
}

/**
 * Hand Menu - Appears on left palm
 */
function HandMenu() {
  return (
    <group>
      {/* Would be positioned relative to hand pose */}
      {/* Implemented with hand tracking integration */}
    </group>
  )
}

