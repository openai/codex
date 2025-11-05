import { useRef } from 'react'
import { Text, Line } from '@react-three/drei'
import * as THREE from 'three'
import { useBranchGraph } from '../hooks/useGitData'

interface BranchStructure3DProps {
  repoPath?: string
}

export default function BranchStructure3D({ repoPath }: BranchStructure3DProps) {
  const { data: branches, isLoading, error } = useBranchGraph(repoPath)
  const groupRef = useRef<THREE.Group>(null)

  if (isLoading) {
    return null
  }

  if (error) {
    return (
      <Text position={[0, 0, 0]} fontSize={2} color="red">
        Error loading branches
      </Text>
    )
  }

  if (!branches || branches.length === 0) {
    return (
      <Text position={[0, 0, 0]} fontSize={1.5} color="yellow">
        No branches found
      </Text>
    )
  }

  return (
    <group ref={groupRef} position={[-20, 0, 0]}>
      {/* Render branch nodes */}
      {branches.map((branch) => (
        <group key={branch.name} position={[branch.x, branch.y / 1000000, branch.z]}>
          {/* Branch sphere */}
          <mesh>
            <sphereGeometry args={[branch.is_active ? 1.5 : 1, 32, 32]} />
            <meshStandardMaterial
              color={branch.is_active ? '#10b981' : '#6b7280'}
              emissive={branch.is_active ? '#10b981' : '#374151'}
              emissiveIntensity={branch.is_active ? 0.5 : 0.2}
            />
          </mesh>

          {/* Branch name */}
          <Text
            position={[0, 2, 0]}
            fontSize={0.5}
            color={branch.is_active ? '#10b981' : 'white'}
            anchorX="center"
            anchorY="bottom"
          >
            {branch.name}
          </Text>

          {/* Merge count indicator */}
          {branch.merge_count > 0 && (
            <Text
              position={[0, -1.5, 0]}
              fontSize={0.3}
              color="#fbbf24"
              anchorX="center"
              anchorY="top"
            >
              {branch.merge_count} merges
            </Text>
          )}
        </group>
      ))}

      {/* Render connections */}
      {branches.map((branch) =>
        branch.connections.map((connection, index) => {
          const targetBranch = branches.find(
            (b) => b.name === connection.target_branch
          )
          if (!targetBranch) return null

          const color =
            connection.connection_type === 'merge'
              ? '#10b981'
              : connection.connection_type === 'fork'
              ? '#f59e0b'
              : '#8b5cf6'

          return (
            <Line
              key={`${branch.name}-${connection.target_branch}-${index}`}
              points={[
                [branch.x, branch.y / 1000000, branch.z],
                [targetBranch.x, targetBranch.y / 1000000, targetBranch.z],
              ]}
              color={color}
              lineWidth={2}
              dashed={connection.connection_type === 'rebase'}
            />
          )
        })
      )}

      {/* Legend */}
      <Text position={[0, -5, 0]} fontSize={0.8} color="white">
        Branch Structure
      </Text>
      <Text position={[0, -6, 0]} fontSize={0.4} color="#10b981">
        Green = Active Branch
      </Text>
    </group>
  )
}

