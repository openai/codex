'use client'

import { useRef, useMemo, useState, useEffect } from 'react'
import { Canvas, useFrame, useThree } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera } from '@react-three/drei'
import * as THREE from 'three'

interface Commit3D {
  sha: string
  message: string
  author: string
  author_email: string
  timestamp: string
  branch: string
  parents: string[]
  x: number
  y: number
  z: number
  color: string
}

interface Scene3DProps {
  commits: Commit3D[]
  onCommitClick?: (commit: Commit3D) => void
  selectedCommitSha?: string
}

function CommitNodesInstanced({
  commits,
  onCommitClick,
  selectedCommitSha,
}: Scene3DProps) {
  const meshRef = useRef<THREE.InstancedMesh>(null)
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null)
  const { camera, raycaster, pointer, gl } = useThree()

  // Prepare instanced mesh data
  const { matrices, colors } = useMemo(() => {
    const matrices: THREE.Matrix4[] = []
    const colors: number[] = []

    commits.forEach((commit) => {
      // Create transformation matrix
      const matrix = new THREE.Matrix4()
      const position = new THREE.Vector3(commit.x, commit.y / 1000000, commit.z)
      const scale = 1.0

      matrix.compose(
        position,
        new THREE.Quaternion(),
        new THREE.Vector3(scale, scale, scale)
      )
      matrices.push(matrix)

      // Parse HSL color
      const color = new THREE.Color(commit.color)
      colors.push(color.r, color.g, color.b)
    })

    return { matrices, colors }
  }, [commits])

  // Set instance matrices and colors
  useEffect(() => {
    if (!meshRef.current) return

    matrices.forEach((matrix, i) => {
      meshRef.current!.setMatrixAt(i, matrix)
    })

    const colorAttr = new THREE.InstancedBufferAttribute(new Float32Array(colors), 3)
    meshRef.current.geometry.setAttribute('color', colorAttr)

    meshRef.current.instanceMatrix.needsUpdate = true
  }, [matrices, colors])

  // Animate selected commit
  useFrame((state) => {
    if (!meshRef.current || !selectedCommitSha) return

    const selectedIndex = commits.findIndex((c) => c.sha === selectedCommitSha)
    if (selectedIndex === -1) return

    const matrix = new THREE.Matrix4()
    meshRef.current.getMatrixAt(selectedIndex, matrix)

    const position = new THREE.Vector3()
    const quaternion = new THREE.Quaternion()
    const scale = new THREE.Vector3()
    matrix.decompose(position, quaternion, scale)

    // Pulse animation
    const pulseScale = 1.2 + Math.sin(state.clock.elapsedTime * 3) * 0.2
    scale.set(pulseScale, pulseScale, pulseScale)

    matrix.compose(position, quaternion, scale)
    meshRef.current.setMatrixAt(selectedIndex, matrix)
    meshRef.current.instanceMatrix.needsUpdate = true
  })

  // Click handling
  const handleClick = (event: THREE.Intersection) => {
    if (event.instanceId !== undefined && onCommitClick) {
      const commit = commits[event.instanceId]
      if (commit) {
        onCommitClick(commit)
      }
    }
  }

  return (
    <instancedMesh
      ref={meshRef}
      args={[undefined, undefined, commits.length]}
      onClick={handleClick}
    >
      <sphereGeometry args={[0.5, 16, 16]} />
      <meshStandardMaterial
        vertexColors
        metalness={0.5}
        roughness={0.3}
      />
    </instancedMesh>
  )
}

function ConnectionLines({ commits }: { commits: Commit3D[] }) {
  const lineSegments = useMemo(() => {
    const points: THREE.Vector3[] = []

    commits.forEach((commit) => {
      const commitPos = new THREE.Vector3(commit.x, commit.y / 1000000, commit.z)

      commit.parents.forEach((parentSha) => {
        const parent = commits.find((c) => c.sha === parentSha)
        if (parent) {
          const parentPos = new THREE.Vector3(parent.x, parent.y / 1000000, parent.z)
          points.push(commitPos)
          points.push(parentPos)
        }
      })
    })

    return points
  }, [commits])

  const geometry = useMemo(() => {
    return new THREE.BufferGeometry().setFromPoints(lineSegments)
  }, [lineSegments])

  return (
    <lineSegments geometry={geometry}>
      <lineBasicMaterial color="#667eea" opacity={0.2} transparent linewidth={1} />
    </lineSegments>
  )
}

export function Scene3D({ commits, onCommitClick, selectedCommitSha }: Scene3DProps) {
  // Normalize Y coordinates
  const normalizedCommits = useMemo(() => {
    if (commits.length === 0) return []

    const minY = Math.min(...commits.map((c) => c.y))
    const maxY = Math.max(...commits.map((c) => c.y))
    const range = maxY - minY || 1

    return commits.map((c) => ({
      ...c,
      y: ((c.y - minY) / range) * 100, // Normalize to 0-100 range
    }))
  }, [commits])

  return (
    <div className="w-full h-full">
      <Canvas>
        <PerspectiveCamera makeDefault position={[50, 50, 50]} />
        <OrbitControls
          enableDamping
          dampingFactor={0.05}
          minDistance={5}
          maxDistance={300}
        />

        {/* Lighting */}
        <ambientLight intensity={0.4} />
        <pointLight position={[50, 50, 50]} intensity={1} color="#ffffff" />
        <pointLight position={[-50, 50, -50]} intensity={0.5} color="#764ba2" />
        <directionalLight position={[0, 100, 0]} intensity={0.3} />

        {/* Commit nodes (Instanced) */}
        {normalizedCommits.length > 0 && (
          <CommitNodesInstanced
            commits={normalizedCommits}
            onCommitClick={onCommitClick}
            selectedCommitSha={selectedCommitSha}
          />
        )}

        {/* Connection lines */}
        {normalizedCommits.length > 0 && <ConnectionLines commits={normalizedCommits} />}

        {/* Grid helper */}
        <gridHelper args={[200, 50, '#444444', '#222222']} rotation={[0, 0, 0]} />

        {/* Axes helper */}
        <axesHelper args={[30]} />

        {/* Stats (FPS counter) */}
        {process.env.NODE_ENV === 'development' && <Stats />}
      </Canvas>
    </div>
  )
}

// FPS Stats component
function Stats() {
  useEffect(() => {
    // @ts-ignore
    import('stats.js').then((StatsModule) => {
      const Stats = StatsModule.default
      const stats = new Stats()
      stats.showPanel(0) // 0: fps, 1: ms, 2: mb
      document.body.appendChild(stats.dom)
      stats.dom.style.position = 'fixed'
      stats.dom.style.top = '80px'
      stats.dom.style.left = '10px'
      stats.dom.style.zIndex = '9999'

      const animate = () => {
        stats.begin()
        stats.end()
        requestAnimationFrame(animate)
      }
      animate()

      return () => {
        document.body.removeChild(stats.dom)
      }
    })
  }, [])

  return null
}
