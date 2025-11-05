import { useRef, useMemo, useEffect } from 'react'
import { useFrame } from '@react-three/fiber'
import { Text, Line } from '@react-three/drei'
import * as THREE from 'three'
import { useCommits } from '../hooks/useGitData'

interface CommitGraph3DLODProps {
  repoPath?: string
}

/**
 * Commit Graph with Level of Detail (LOD) optimization
 * Automatically switches between high/medium/low detail based on camera distance
 */
export default function CommitGraph3DLOD({ repoPath }: CommitGraph3DLODProps) {
  const { data: commits, isLoading, error } = useCommits(repoPath)
  const groupRef = useRef<THREE.Group>(null)
  const lodRef = useRef<Map<string, THREE.LOD>>(new Map())

  // Normalize coordinates
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

  // Create LOD geometries
  const lodGeometries = useMemo(() => {
    return {
      high: new THREE.SphereGeometry(0.5, 32, 32), // High detail
      medium: new THREE.SphereGeometry(0.5, 16, 16), // Medium detail
      low: new THREE.SphereGeometry(0.5, 8, 8), // Low detail
      billboard: new THREE.PlaneGeometry(1, 1), // Distant billboard
    }
  }, [])

  // Create LOD objects for each commit
  useEffect(() => {
    if (!normalizedCommits.length || !groupRef.current) return

    // Clear existing LODs
    lodRef.current.forEach((lod) => {
      groupRef.current?.remove(lod)
    })
    lodRef.current.clear()

    normalizedCommits.forEach((commit) => {
      const lod = new THREE.LOD()

      // High detail mesh (0-30 units)
      const highMaterial = new THREE.MeshStandardMaterial({
        color: commit.color,
        emissive: commit.color,
        emissiveIntensity: 0.3,
      })
      const highMesh = new THREE.Mesh(lodGeometries.high, highMaterial)
      lod.addLevel(highMesh, 0)

      // Medium detail mesh (30-60 units)
      const mediumMaterial = new THREE.MeshStandardMaterial({
        color: commit.color,
        emissive: commit.color,
        emissiveIntensity: 0.2,
      })
      const mediumMesh = new THREE.Mesh(lodGeometries.medium, mediumMaterial)
      lod.addLevel(mediumMesh, 30)

      // Low detail mesh (60-100 units)
      const lowMaterial = new THREE.MeshStandardMaterial({
        color: commit.color,
        emissive: commit.color,
        emissiveIntensity: 0.1,
      })
      const lowMesh = new THREE.Mesh(lodGeometries.low, lowMaterial)
      lod.addLevel(lowMesh, 60)

      // Billboard sprite (100+ units)
      const billboardMaterial = new THREE.SpriteMaterial({
        color: commit.color,
        sizeAttenuation: true,
      })
      const billboard = new THREE.Sprite(billboardMaterial)
      billboard.scale.set(0.5, 0.5, 0.5)
      lod.addLevel(billboard, 100)

      // Position LOD
      lod.position.set(commit.x, commit.normalizedY, commit.z)

      groupRef.current?.add(lod)
      lodRef.current.set(commit.sha, lod)
    })

    return () => {
      // Cleanup
      Object.values(lodGeometries).forEach((geo) => geo.dispose())
    }
  }, [normalizedCommits, lodGeometries])

  // Update LOD levels based on camera position
  useFrame(({ camera }) => {
    lodRef.current.forEach((lod) => {
      lod.update(camera)
    })

    // Gentle rotation
    if (groupRef.current) {
      groupRef.current.rotation.y += 0.001
    }
  })

  // Render connections (limited for performance)
  const commitConnections = useMemo(() => {
    if (!normalizedCommits.length) return []

    const commitMap = new Map(normalizedCommits.map((c) => [c.sha, c]))
    const connections: Array<{
      from: [number, number, number]
      to: [number, number, number]
    }> = []

    const maxConnections = 300 // Reduced for LOD
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
      {/* LOD objects are managed in useEffect */}

      {/* Render connections */}
      {commitConnections.map((connection, index) => (
        <Line
          key={`line-${index}`}
          points={[connection.from, connection.to]}
          color="rgba(56, 189, 248, 0.2)"
          lineWidth={0.5}
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

