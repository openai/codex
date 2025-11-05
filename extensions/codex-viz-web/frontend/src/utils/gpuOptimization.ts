/**
 * GPU Optimization Utilities
 * Best practices for WebGL/Three.js performance
 */

import * as THREE from 'three'

/**
 * Check if GPU acceleration is available and performant
 */
export function checkGPUCapabilities(): {
  isSupported: boolean
  renderer: string
  maxTextureSize: number
  maxVertexUniforms: number
  isHighPerformance: boolean
} {
  const canvas = document.createElement('canvas')
  const gl = canvas.getContext('webgl2') || canvas.getContext('webgl')

  if (!gl) {
    return {
      isSupported: false,
      renderer: 'unknown',
      maxTextureSize: 0,
      maxVertexUniforms: 0,
      isHighPerformance: false,
    }
  }

  const debugInfo = gl.getExtension('WEBGL_debug_renderer_info')
  const renderer = debugInfo
    ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL)
    : 'unknown'

  const maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE)
  const maxVertexUniforms = gl.getParameter(gl.MAX_VERTEX_UNIFORM_VECTORS)

  // Check if it's a high-performance GPU
  const isHighPerformance =
    !renderer.toLowerCase().includes('intel') &&
    maxTextureSize >= 8192 &&
    maxVertexUniforms >= 256

  return {
    isSupported: true,
    renderer,
    maxTextureSize,
    maxVertexUniforms,
    isHighPerformance,
  }
}

/**
 * Configure renderer for optimal performance
 */
export function configureHighPerformanceRenderer(
  renderer: THREE.WebGLRenderer
): void {
  // Enable hardware acceleration hints
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))

  // Optimize shadow maps
  renderer.shadowMap.enabled = false // Disable for performance
  renderer.shadowMap.type = THREE.PCFSoftShadowMap

  // Enable frustum culling
  renderer.info.autoReset = true

  // Use optimal color space
  renderer.outputColorSpace = THREE.SRGBColorSpace

  // Optimize canvas
  const canvas = renderer.domElement
  canvas.style.imageRendering = 'auto'
  canvas.style.webkitFontSmoothing = 'antialiased'
}

/**
 * Create geometry with optimal buffer attributes
 */
export function createOptimizedBufferGeometry(
  positions: Float32Array,
  normals?: Float32Array,
  colors?: Float32Array
): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry()

  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))

  if (normals) {
    geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3))
  } else {
    geometry.computeVertexNormals()
  }

  if (colors) {
    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  }

  // Compute bounding sphere for frustum culling
  geometry.computeBoundingSphere()

  return geometry
}

/**
 * Create instanced mesh for large numbers of identical objects
 */
export function createInstancedMesh(
  geometry: THREE.BufferGeometry,
  material: THREE.Material,
  count: number,
  positions: Float32Array,
  colors?: Float32Array
): THREE.InstancedMesh {
  const mesh = new THREE.InstancedMesh(geometry, material, count)

  const matrix = new THREE.Matrix4()
  const color = new THREE.Color()

  for (let i = 0; i < count; i++) {
    // Set position
    matrix.setPosition(
      positions[i * 3],
      positions[i * 3 + 1],
      positions[i * 3 + 2]
    )
    mesh.setMatrixAt(i, matrix)

    // Set color if provided
    if (colors) {
      color.setRGB(colors[i * 3], colors[i * 3 + 1], colors[i * 3 + 2])
      mesh.setColorAt(i, color)
    }
  }

  mesh.instanceMatrix.needsUpdate = true
  if (mesh.instanceColor) {
    mesh.instanceColor.needsUpdate = true
  }

  // Enable frustum culling per instance
  mesh.frustumCulled = true

  return mesh
}

/**
 * Level of Detail (LOD) helper
 */
export function createLOD(
  positions: Float32Array,
  highDetailGeometry: THREE.BufferGeometry,
  midDetailGeometry: THREE.BufferGeometry,
  lowDetailGeometry: THREE.BufferGeometry,
  material: THREE.Material
): THREE.LOD {
  const lod = new THREE.LOD()

  // High detail (close)
  const highMesh = new THREE.Mesh(highDetailGeometry, material)
  lod.addLevel(highMesh, 0)

  // Mid detail (medium distance)
  const midMesh = new THREE.Mesh(midDetailGeometry, material)
  lod.addLevel(midMesh, 50)

  // Low detail (far)
  const lowMesh = new THREE.Mesh(lowDetailGeometry, material)
  lod.addLevel(lowMesh, 100)

  return lod
}

/**
 * Dispose geometry and material properly to free GPU memory
 */
export function disposeObject(object: THREE.Object3D): void {
  object.traverse((child) => {
    if (child instanceof THREE.Mesh) {
      child.geometry?.dispose()

      if (Array.isArray(child.material)) {
        child.material.forEach((material) => material.dispose())
      } else {
        child.material?.dispose()
      }
    }
  })
}

/**
 * Monitor GPU memory usage (approximation)
 */
export function estimateGPUMemoryUsage(scene: THREE.Scene): {
  geometries: number
  textures: number
  total: number
} {
  let geometryMemory = 0
  let textureMemory = 0

  scene.traverse((object) => {
    if (object instanceof THREE.Mesh) {
      const geometry = object.geometry
      if (geometry) {
        // Estimate geometry memory
        const positionAttr = geometry.getAttribute('position')
        if (positionAttr) {
          geometryMemory += positionAttr.array.byteLength
        }
      }

      // Estimate texture memory
      const material = object.material as THREE.Material
      if (material && 'map' in material) {
        const texture = (material as any).map as THREE.Texture
        if (texture && texture.image) {
          const { width, height } = texture.image
          textureMemory += width * height * 4 // RGBA
        }
      }
    }
  })

  return {
    geometries: geometryMemory / 1024 / 1024, // MB
    textures: textureMemory / 1024 / 1024, // MB
    total: (geometryMemory + textureMemory) / 1024 / 1024, // MB
  }
}

