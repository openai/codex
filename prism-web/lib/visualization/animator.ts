/**
 * Animation System
 * 
 * Time-based animation interpolation and camera path playback
 */

import * as THREE from 'three'

export interface CameraKeyframe {
  position: THREE.Vector3
  lookAt: THREE.Vector3
  timestamp: number
}

export interface AnimationConfig {
  duration: number // milliseconds
  easing: (t: number) => number
  loop: boolean
}

export class CameraAnimator {
  private keyframes: CameraKeyframe[] = []
  private playing: boolean = false
  private currentTime: number = 0
  private config: AnimationConfig

  constructor(config?: Partial<AnimationConfig>) {
    this.config = {
      duration: 10000,
      easing: this.easeInOutCubic,
      loop: false,
      ...config,
    }
  }

  /**
   * Add camera keyframe
   */
  addKeyframe(keyframe: CameraKeyframe) {
    this.keyframes.push(keyframe)
    this.keyframes.sort((a, b) => a.timestamp - b.timestamp)
  }

  /**
   * Start playback
   */
  play() {
    this.playing = true
    this.currentTime = 0
  }

  /**
   * Pause playback
   */
  pause() {
    this.playing = false
  }

  /**
   * Stop playback and reset
   */
  stop() {
    this.playing = false
    this.currentTime = 0
  }

  /**
   * Seek to specific time
   */
  seek(time: number) {
    this.currentTime = Math.max(0, Math.min(time, this.config.duration))
  }

  /**
   * Update animation (call in animation loop)
   */
  update(deltaTime: number, camera: THREE.Camera): boolean {
    if (!this.playing || this.keyframes.length < 2) {
      return false
    }

    this.currentTime += deltaTime

    if (this.currentTime >= this.config.duration) {
      if (this.config.loop) {
        this.currentTime = 0
      } else {
        this.playing = false
        return false
      }
    }

    // Interpolate camera position
    const progress = this.currentTime / this.config.duration
    const easedProgress = this.config.easing(progress)

    const { position, lookAt } = this.interpolateKeyframes(easedProgress)

    camera.position.copy(position)
    camera.lookAt(lookAt)
    camera.updateProjectionMatrix()

    return true
  }

  /**
   * Interpolate between keyframes
   */
  private interpolateKeyframes(t: number): { position: THREE.Vector3; lookAt: THREE.Vector3 } {
    if (this.keyframes.length === 0) {
      return {
        position: new THREE.Vector3(),
        lookAt: new THREE.Vector3(),
      }
    }

    if (this.keyframes.length === 1 || t <= 0) {
      return {
        position: this.keyframes[0].position.clone(),
        lookAt: this.keyframes[0].lookAt.clone(),
      }
    }

    if (t >= 1) {
      const last = this.keyframes[this.keyframes.length - 1]
      return {
        position: last.position.clone(),
        lookAt: last.lookAt.clone(),
      }
    }

    // Find surrounding keyframes
    const totalDuration = this.keyframes[this.keyframes.length - 1].timestamp - this.keyframes[0].timestamp
    const currentTimestamp = this.keyframes[0].timestamp + totalDuration * t

    let beforeIndex = 0
    let afterIndex = 1

    for (let i = 0; i < this.keyframes.length - 1; i++) {
      if (
        currentTimestamp >= this.keyframes[i].timestamp &&
        currentTimestamp <= this.keyframes[i + 1].timestamp
      ) {
        beforeIndex = i
        afterIndex = i + 1
        break
      }
    }

    const before = this.keyframes[beforeIndex]
    const after = this.keyframes[afterIndex]

    const segmentDuration = after.timestamp - before.timestamp
    const segmentProgress =
      segmentDuration > 0 ? (currentTimestamp - before.timestamp) / segmentDuration : 0

    // Lerp position and lookAt
    const position = new THREE.Vector3().lerpVectors(
      before.position,
      after.position,
      segmentProgress
    )

    const lookAt = new THREE.Vector3().lerpVectors(
      before.lookAt,
      after.lookAt,
      segmentProgress
    )

    return { position, lookAt }
  }

  /**
   * Cubic ease in-out function
   */
  private easeInOutCubic(t: number): number {
    return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2
  }

  /**
   * Linear easing
   */
  static easeLinear(t: number): number {
    return t
  }

  /**
   * Quadratic ease in-out
   */
  static easeInOutQuad(t: number): number {
    return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2
  }

  /**
   * Get playback state
   */
  isPlaying(): boolean {
    return this.playing
  }

  /**
   * Get current progress (0-1)
   */
  getProgress(): number {
    return this.config.duration > 0 ? this.currentTime / this.config.duration : 0
  }

  /**
   * Get total duration
   */
  getDuration(): number {
    return this.config.duration
  }

  /**
   * Set duration
   */
  setDuration(duration: number) {
    this.config.duration = duration
  }

  /**
   * Clear all keyframes
   */
  clearKeyframes() {
    this.keyframes = []
  }

  /**
   * Get keyframe count
   */
  getKeyframeCount(): number {
    return this.keyframes.length
  }
}

/**
 * Frustum Culling Helper
 */
export class FrustumCuller {
  private frustum: THREE.Frustum
  private projectionMatrix: THREE.Matrix4

  constructor() {
    this.frustum = new THREE.Frustum()
    this.projectionMatrix = new THREE.Matrix4()
  }

  /**
   * Update frustum from camera
   */
  updateFromCamera(camera: THREE.Camera) {
    this.projectionMatrix.multiplyMatrices(
      camera.projectionMatrix,
      camera.matrixWorldInverse
    )
    this.frustum.setFromProjectionMatrix(this.projectionMatrix)
  }

  /**
   * Test if point is in frustum
   */
  isPointVisible(point: THREE.Vector3): boolean {
    return this.frustum.containsPoint(point)
  }

  /**
   * Test if sphere is in frustum
   */
  isSphereVisible(center: THREE.Vector3, radius: number): boolean {
    const sphere = new THREE.Sphere(center, radius)
    return this.frustum.intersectsSphere(sphere)
  }

  /**
   * Test if box is in frustum
   */
  isBoxVisible(box: THREE.Box3): boolean {
    return this.frustum.intersectsBox(box)
  }

  /**
   * Filter visible objects
   */
  filterVisible<T extends { position: THREE.Vector3 }>(
    objects: T[],
    radius: number = 0.5
  ): T[] {
    return objects.filter((obj) => this.isSphereVisible(obj.position, radius))
  }
}
