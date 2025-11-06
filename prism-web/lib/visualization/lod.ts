/**
 * Level of Detail (LOD) System
 * 
 * Dynamically adjusts rendering detail based on camera distance
 */

import * as THREE from 'three'

export enum LODLevel {
  High = 0,    // < 50 units - full detail
  Medium = 1,  // 50-200 units - reduced detail
  Low = 2,     // > 200 units - minimal detail
}

export interface LODConfig {
  highDetailDistance: number
  mediumDetailDistance: number
  highGeometry: THREE.BufferGeometry
  mediumGeometry: THREE.BufferGeometry
  lowGeometry: THREE.BufferGeometry
}

export class LODManager {
  private config: LODConfig
  private currentLevel: LODLevel = LODLevel.High

  constructor(config?: Partial<LODConfig>) {
    this.config = {
      highDetailDistance: 50,
      mediumDetailDistance: 200,
      highGeometry: new THREE.SphereGeometry(0.5, 32, 32),
      mediumGeometry: new THREE.SphereGeometry(0.5, 16, 16),
      lowGeometry: new THREE.SphereGeometry(0.5, 8, 8),
      ...config,
    }
  }

  /**
   * Get appropriate LOD level based on distance
   */
  getLODLevel(distance: number): LODLevel {
    if (distance < this.config.highDetailDistance) {
      return LODLevel.High
    } else if (distance < this.config.mediumDetailDistance) {
      return LODLevel.Medium
    } else {
      return LODLevel.Low
    }
  }

  /**
   * Get geometry for LOD level
   */
  getGeometry(level: LODLevel): THREE.BufferGeometry {
    switch (level) {
      case LODLevel.High:
        return this.config.highGeometry
      case LODLevel.Medium:
        return this.config.mediumGeometry
      case LODLevel.Low:
        return this.config.lowGeometry
    }
  }

  /**
   * Update current LOD level
   */
  updateLOD(distance: number): { changed: boolean; level: LODLevel } {
    const newLevel = this.getLODLevel(distance)
    const changed = newLevel !== this.currentLevel

    if (changed) {
      this.currentLevel = newLevel
    }

    return { changed, level: newLevel }
  }

  /**
   * Get segment count for LOD level
   */
  getSegmentCount(level: LODLevel): { widthSegments: number; heightSegments: number } {
    switch (level) {
      case LODLevel.High:
        return { widthSegments: 32, heightSegments: 32 }
      case LODLevel.Medium:
        return { widthSegments: 16, heightSegments: 16 }
      case LODLevel.Low:
        return { widthSegments: 8, heightSegments: 8 }
    }
  }

  /**
   * Dispose all geometries
   */
  dispose() {
    this.config.highGeometry.dispose()
    this.config.mediumGeometry.dispose()
    this.config.lowGeometry.dispose()
  }
}

/**
 * Octree for spatial culling
 */
export class Octree {
  private root: OctreeNode
  private maxDepth: number
  private maxObjects: number

  constructor(
    bounds: THREE.Box3,
    maxDepth: number = 5,
    maxObjects: number = 10
  ) {
    this.root = new OctreeNode(bounds, 0)
    this.maxDepth = maxDepth
    this.maxObjects = maxObjects
  }

  /**
   * Insert an object into the octree
   */
  insert(object: { position: THREE.Vector3; data: any }) {
    this.root.insert(object, this.maxDepth, this.maxObjects)
  }

  /**
   * Query objects within frustum
   */
  queryFrustum(frustum: THREE.Frustum): any[] {
    const result: any[] = []
    this.root.queryFrustum(frustum, result)
    return result
  }

  /**
   * Clear all objects
   */
  clear() {
    this.root.clear()
  }
}

class OctreeNode {
  private bounds: THREE.Box3
  private depth: number
  private objects: Array<{ position: THREE.Vector3; data: any }> = []
  private children: OctreeNode[] | null = null

  constructor(bounds: THREE.Box3, depth: number) {
    this.bounds = bounds
    this.depth = depth
  }

  insert(
    object: { position: THREE.Vector3; data: any },
    maxDepth: number,
    maxObjects: number
  ) {
    if (this.children !== null) {
      // Delegate to children
      const octant = this.getOctant(object.position)
      if (octant >= 0) {
        this.children[octant].insert(object, maxDepth, maxObjects)
      }
      return
    }

    // Add to this node
    this.objects.push(object)

    // Subdivide if needed
    if (
      this.objects.length > maxObjects &&
      this.depth < maxDepth
    ) {
      this.subdivide()
      
      // Redistribute objects to children
      const objectsToRedistribute = [...this.objects]
      this.objects = []
      
      for (const obj of objectsToRedistribute) {
        const octant = this.getOctant(obj.position)
        if (octant >= 0 && this.children) {
          this.children[octant].insert(obj, maxDepth, maxObjects)
        } else {
          this.objects.push(obj)
        }
      }
    }
  }

  queryFrustum(frustum: THREE.Frustum, result: any[]) {
    // Check if bounds intersect frustum
    if (!frustum.intersectsBox(this.bounds)) {
      return
    }

    // Add objects in this node
    result.push(...this.objects.map((o) => o.data))

    // Query children
    if (this.children) {
      for (const child of this.children) {
        child.queryFrustum(frustum, result)
      }
    }
  }

  clear() {
    this.objects = []
    if (this.children) {
      for (const child of this.children) {
        child.clear()
      }
      this.children = null
    }
  }

  private subdivide() {
    const center = new THREE.Vector3()
    this.bounds.getCenter(center)

    const min = this.bounds.min
    const max = this.bounds.max

    this.children = [
      // Bottom octants
      new OctreeNode(new THREE.Box3(min, center), this.depth + 1),
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(center.x, min.y, min.z),
          new THREE.Vector3(max.x, center.y, center.z)
        ),
        this.depth + 1
      ),
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(min.x, min.y, center.z),
          new THREE.Vector3(center.x, center.y, max.z)
        ),
        this.depth + 1
      ),
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(center.x, min.y, center.z),
          new THREE.Vector3(max.x, center.y, max.z)
        ),
        this.depth + 1
      ),
      // Top octants
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(min.x, center.y, min.z),
          new THREE.Vector3(center.x, max.y, center.z)
        ),
        this.depth + 1
      ),
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(center.x, center.y, min.z),
          new THREE.Vector3(max.x, max.y, center.z)
        ),
        this.depth + 1
      ),
      new OctreeNode(
        new THREE.Box3(
          new THREE.Vector3(min.x, center.y, center.z),
          new THREE.Vector3(center.x, max.y, max.z)
        ),
        this.depth + 1
      ),
      new OctreeNode(
        new THREE.Box3(center, max),
        this.depth + 1
      ),
    ]
  }

  private getOctant(position: THREE.Vector3): number {
    const center = new THREE.Vector3()
    this.bounds.getCenter(center)

    let index = 0
    if (position.x >= center.x) index |= 1
    if (position.y >= center.y) index |= 4
    if (position.z >= center.z) index |= 2

    return index
  }
}
