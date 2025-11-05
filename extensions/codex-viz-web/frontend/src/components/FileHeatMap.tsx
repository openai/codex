import { useRef, useMemo } from 'react'
import { Text } from '@react-three/drei'
import * as THREE from 'three'
import { useFileHeatmap } from '../hooks/useGitData'

interface FileHeatMapProps {
  repoPath?: string
}

export default function FileHeatMap({ repoPath }: FileHeatMapProps) {
  const { data: fileStats, isLoading, error } = useFileHeatmap(repoPath)
  const groupRef = useRef<THREE.Group>(null)

  // Create heatmap visualization
  const heatmapData = useMemo(() => {
    if (!fileStats || fileStats.length === 0) return []

    const gridSize = Math.ceil(Math.sqrt(fileStats.length))
    
    return fileStats.map((file, index) => {
      const x = (index % gridSize) * 2
      const z = Math.floor(index / gridSize) * 2
      const y = file.heat_level * 10 // Height based on heat level

      // Color from blue (cold) to red (hot)
      const color = new THREE.Color()
      color.setHSL(0.6 - file.heat_level * 0.6, 1, 0.5)

      return {
        ...file,
        position: [x - gridSize, y, z - gridSize] as [number, number, number],
        color: color.getHex(),
      }
    })
  }, [fileStats])

  if (isLoading) {
    return null
  }

  if (error) {
    return (
      <Text position={[0, 0, 0]} fontSize={2} color="red">
        Error loading file stats
      </Text>
    )
  }

  if (!heatmapData.length) {
    return (
      <Text position={[0, 0, 0]} fontSize={1.5} color="yellow">
        No file data found
      </Text>
    )
  }

  return (
    <group ref={groupRef} position={[20, 0, 0]}>
      {heatmapData.map((file) => (
        <group key={file.path} position={file.position}>
          {/* File box */}
          <mesh>
            <boxGeometry args={[1.5, file.position[1] || 0.5, 1.5]} />
            <meshStandardMaterial
              color={file.color}
              emissive={file.color}
              emissiveIntensity={0.2}
              transparent
              opacity={0.8}
            />
          </mesh>

          {/* File path label (for significant files) */}
          {file.heat_level > 0.5 && (
            <Text
              position={[0, (file.position[1] || 0) + 1, 0]}
              fontSize={0.2}
              color="white"
              anchorX="center"
              anchorY="bottom"
              maxWidth={3}
            >
              {file.path.split('/').pop() || file.path}
            </Text>
          )}
        </group>
      ))}

      {/* Legend */}
      <Text position={[0, -3, 0]} fontSize={0.8} color="white">
        File Change Heatmap
      </Text>
      <Text position={[0, -4, 0]} fontSize={0.4} color="#60A5FA">
        Height = Change Frequency
      </Text>
    </group>
  )
}

