import { useRef, useMemo, useEffect } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text, Line } from '@react-three/drei'
import * as THREE from 'three'
import { useCommits } from '../hooks/useGitData'

interface CommitGraph3DOptimizedProps {
  repoPath?: string
}

export default function CommitGraph3DOptimized({ repoPath }: CommitGraph3DOptimizedProps) {
  const { data: commits, isLoading, error } = useCommits(repoPath)
  const groupRef = useRef<THREE.Group>(null)
  const instancedMeshRef = useRef<THREE.InstancedMesh>(null)

  // Normalize coordinates for better visualization
  const normalizedCommits = useMemo(() => {
    if (!commits || commits.length === 0) return []

    const minY = Math.min(...commits.map((c) => c.y))
    const maxY = Math.max(...commits.map((c) => c.y))
    const timeRange = maxY - minY || 1

    return commits.map((commit) => ({
      ...commit,
      normalizedY: ((commit.y - minY) / timeRange) * 100,
    }))
  }, [commits])

  // GPU-optimized instanced rendering
  useEffect(() => {
    if (!instancedMeshRef.current || normalizedCommits.length === 0) return

    const mesh = instancedMeshRef.current
    const tempObject = new THREE.Object3D()
    const tempColor = new THREE.Color()

    normalizedCommits.forEach((commit, i) => {
      // Set position
      tempObject.position.set(commit.x, commit.normalizedY, commit.z)
      
      // Set scale (slightly random for visual interest)
      const scale = 0.5 + Math.random() * 0.2
      tempObject.scale.set(scale, scale, scale)
      
      // Update matrix
      tempObject.updateMatrix()
      mesh.setMatrixAt(i, tempObject.matrix)

      // Set color
      tempColor.set(commit.color)
      mesh.setColorAt(i, tempColor)
    })

    mesh.instanceMatrix.needsUpdate = true
    if (mesh.instanceColor) {
      mesh.instanceColor.needsUpdate = true
    }
  }, [normalizedCommits])

  // Gentle rotation animation
  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y += 0.001
    }
  })

  // Create connections between commits (using geometry pooling)
  const commitConnections = useMemo(() => {
    if (!normalizedCommits.length) return []

    const commitMap = new Map(normalizedCommits.map((c) => [c.sha, c]))
    const connections: Array<{
      from: [number, number, number]
      to: [number, number, number]
    }> = []

    // Limit connections for performance
    const maxConnections = 500
    let connectionCount = 0

    for (const commit of normalizedCommits) {
      if (connectionCount >= maxConnections) break

      for (const parentSha of commit.parents) {
        const parent = commitMap.get(parentSha)
        if (parent) {
          connections.push({
            from: [commit.x, commit.normalizedY, commit.z],
            to: [parent.x, parent.normalizedY, parent.z],
          })
          connectionCount++
          if (connectionCount >= maxConnections) break
        }
      }
    }

    return connections
  }, [normalizedCommits])

  if (isLoading) {
    return null
  }

  if (error) {
    return (
      <Text position={[0, 0, 0]} fontSize={2} color="red">
        Error loading commits
      </Text>
    )
  }

  if (!normalizedCommits.length) {
    return (
      <Text position={[0, 0, 0]} fontSize={1.5} color="yellow">
        No commits found
      </Text>
    )
  }

  return (
    <group ref={groupRef}>
      {/* GPU-accelerated instanced rendering for commits */}
      <instancedMesh
        ref={instancedMeshRef}
        args={[undefined, undefined, normalizedCommits.length]}
        frustumCulled={true}
      >
        <sphereGeometry args={[0.5, 16, 16]} />
        <meshStandardMaterial
          toneMapped={false}
          emissive="#38bdf8"
          emissiveIntensity={0.3}
        />
      </instancedMesh>

      {/* Render connections (batched) */}
      {commitConnections.map((connection, index) => (
        <Line
          key={`line-${index}`}
          points={[connection.from, connection.to]}
          color="rgba(56, 189, 248, 0.3)"
          lineWidth={1}
          dashed={false}
        />
      ))}

      {/* Axis labels */}
      <Text position={[0, -5, 0]} fontSize={1} color="white">
        Branch Axis
      </Text>
      <Text position={[0, 110, 0]} fontSize={1} color="white">
        Time Axis →
      </Text>
    </group>
  )
}

