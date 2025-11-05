import { useRef, useMemo } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text, Line } from '@react-three/drei'
import * as THREE from 'three'
import { useCommits } from '../hooks/useGitData'

interface CommitGraph3DProps {
  repoPath?: string
}

export default function CommitGraph3D({ repoPath }: CommitGraph3DProps) {
  const { data: commits, isLoading, error } = useCommits(repoPath)
  const groupRef = useRef<THREE.Group>(null)

  // Gentle rotation animation
  useFrame((state) => {
    if (groupRef.current) {
      groupRef.current.rotation.y += 0.001
    }
  })

  // Normalize coordinates for better visualization
  const normalizedCommits = useMemo(() => {
    if (!commits || commits.length === 0) return []

    const minY = Math.min(...commits.map((c) => c.y))
    const maxY = Math.max(...commits.map((c) => c.y))
    const timeRange = maxY - minY || 1

    return commits.map((commit) => ({
      ...commit,
      // Normalize time to 0-100 range
      normalizedY: ((commit.y - minY) / timeRange) * 100,
    }))
  }, [commits])

  // Create connections between commits and their parents
  const commitConnections = useMemo(() => {
    if (!normalizedCommits.length) return []

    const commitMap = new Map(normalizedCommits.map((c) => [c.sha, c]))
    const connections: Array<{
      from: [number, number, number]
      to: [number, number, number]
    }> = []

    normalizedCommits.forEach((commit) => {
      commit.parents.forEach((parentSha) => {
        const parent = commitMap.get(parentSha)
        if (parent) {
          connections.push({
            from: [commit.x, commit.normalizedY, commit.z],
            to: [parent.x, parent.normalizedY, parent.z],
          })
        }
      })
    })

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
      {/* Render commit nodes */}
      {normalizedCommits.map((commit, index) => (
        <group
          key={commit.sha}
          position={[commit.x, commit.normalizedY, commit.z]}
        >
          <mesh>
            <sphereGeometry args={[0.5, 16, 16]} />
            <meshStandardMaterial
              color={commit.color}
              emissive={commit.color}
              emissiveIntensity={0.3}
            />
          </mesh>

          {/* Commit message on hover (simplified) */}
          {index % 10 === 0 && (
            <Text
              position={[0, 1.5, 0]}
              fontSize={0.3}
              color="white"
              anchorX="center"
              anchorY="middle"
              maxWidth={10}
            >
              {commit.message.substring(0, 30)}
            </Text>
          )}
        </group>
      ))}

      {/* Render connections between commits */}
      {commitConnections.map((connection, index) => (
        <Line
          key={`line-${index}`}
          points={[connection.from, connection.to]}
          color="rgba(255, 255, 255, 0.2)"
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

