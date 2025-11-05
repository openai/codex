/**
 * VR Navigation System
 * Teleportation, Smooth Locomotion, Comfort options
 */

import * as THREE from 'three'

export type LocomotionMode = 'teleport' | 'smooth' | 'snap-turn'

export interface ComfortOptions {
  vignette: boolean          // Reduce FOV during movement
  tunneling: boolean         // Edge blurring
  snapTurnAngle: number      // Degrees per snap (15, 30, 45)
  smoothSpeed: number        // m/s for smooth locomotion
}

export class VRNavigator {
  private mode: LocomotionMode = 'teleport'
  private comfort: ComfortOptions = {
    vignette: true,
    tunneling: true,
    snapTurnAngle: 30,
    smoothSpeed: 3.0,
  }

  private teleportMarker: THREE.Object3D | null = null
  private currentPosition = new THREE.Vector3(0, 0, 0)
  private currentRotation = 0

  constructor(comfortOptions?: Partial<ComfortOptions>) {
    if (comfortOptions) {
      this.comfort = { ...this.comfort, ...comfortOptions }
    }
  }

  /**
   * Set locomotion mode
   */
  setMode(mode: LocomotionMode): void {
    this.mode = mode
    console.log(`VR Navigator: Mode set to ${mode}`)
  }

  /**
   * Update navigation (called every frame)
   */
  update(
    delta: number,
    player: THREE.Group,
    inputSources: XRInputSource[]
  ): void {
    switch (this.mode) {
      case 'teleport':
        this.updateTeleport(player, inputSources)
        break
      case 'smooth':
        this.updateSmooth(delta, player, inputSources)
        break
      case 'snap-turn':
        this.updateSnapTurn(player, inputSources)
        break
    }
  }

  /**
   * Teleportation movement
   */
  private updateTeleport(player: THREE.Group, inputSources: XRInputSource[]): void {
    for (const inputSource of inputSources) {
      if (inputSource.handedness !== 'left') continue

      const gamepad = inputSource.gamepad
      if (!gamepad) continue

      // Thumbstick Y axis for teleport arc
      const thumbstickY = gamepad.axes[3] || 0

      if (Math.abs(thumbstickY) > 0.5) {
        // Show teleport indicator
        this.showTeleportMarker(player, thumbstickY)
      } else {
        this.hideTeleportMarker()
      }

      // Trigger button to confirm teleport
      if (gamepad.buttons[0]?.pressed && this.teleportMarker) {
        this.executeTeleport(player)
      }
    }
  }

  /**
   * Smooth locomotion
   */
  private updateSmooth(delta: number, player: THREE.Group, inputSources: XRInputSource[]): void {
    for (const inputSource of inputSources) {
      if (inputSource.handedness !== 'left') continue

      const gamepad = inputSource.gamepad
      if (!gamepad) continue

      const thumbstickX = gamepad.axes[2] || 0
      const thumbstickY = gamepad.axes[3] || 0

      if (Math.abs(thumbstickX) > 0.1 || Math.abs(thumbstickY) > 0.1) {
        // Calculate movement direction
        const direction = new THREE.Vector3(thumbstickX, 0, -thumbstickY)
        direction.applyQuaternion(player.quaternion)
        direction.multiplyScalar(this.comfort.smoothSpeed * delta)

        player.position.add(direction)

        // Apply vignette effect if enabled
        if (this.comfort.vignette) {
          this.applyVignetteEffect(Math.abs(thumbstickX) + Math.abs(thumbstickY))
        }
      }
    }
  }

  /**
   * Snap turning (comfort mode)
   */
  private updateSnapTurn(player: THREE.Group, inputSources: XRInputSource[]): void {
    for (const inputSource of inputSources) {
      if (inputSource.handedness !== 'right') continue

      const gamepad = inputSource.gamepad
      if (!gamepad) continue

      const thumbstickX = gamepad.axes[2] || 0

      // Detect snap turn trigger
      if (Math.abs(thumbstickX) > 0.8) {
        const turnDirection = thumbstickX > 0 ? 1 : -1
        const turnAngle = (this.comfort.snapTurnAngle * Math.PI / 180) * turnDirection

        player.rotateY(turnAngle)

        // Add small delay to prevent multiple snaps
        this.currentRotation += turnAngle
      }
    }
  }

  /**
   * Show teleport destination marker
   */
  private showTeleportMarker(player: THREE.Group, distance: number): void {
    if (!this.teleportMarker) {
      // Create marker (would be a disc or arrow)
      const geometry = new THREE.CircleGeometry(0.5, 32)
      const material = new THREE.MeshBasicMaterial({ 
        color: 0x00ff00,
        opacity: 0.5,
        transparent: true,
      })
      this.teleportMarker = new THREE.Mesh(geometry, material)
      this.teleportMarker.rotation.x = -Math.PI / 2
    }

    // Calculate teleport position (parabolic arc)
    const forward = new THREE.Vector3(0, 0, -1)
    forward.applyQuaternion(player.quaternion)
    forward.multiplyScalar(Math.abs(distance) * 5)

    this.teleportMarker.position.copy(player.position)
    this.teleportMarker.position.add(forward)
    this.teleportMarker.position.y = 0  // Snap to ground

    // Add to scene if not already added
    if (this.teleportMarker.parent !== player.parent) {
      player.parent?.add(this.teleportMarker)
    }
  }

  /**
   * Hide teleport marker
   */
  private hideTeleportMarker(): void {
    if (this.teleportMarker && this.teleportMarker.parent) {
      this.teleportMarker.parent.remove(this.teleportMarker)
    }
  }

  /**
   * Execute teleport to marker position
   */
  private executeTeleport(player: THREE.Group): void {
    if (!this.teleportMarker) return

    player.position.copy(this.teleportMarker.position)
    player.position.y = 0  // Player height handled by XR system

    this.hideTeleportMarker()

    console.log('VR Navigator: Teleported to', player.position)
  }

  /**
   * Apply vignette effect for comfort
   */
  private applyVignetteEffect(intensity: number): void {
    // This would be implemented as a post-processing effect
    // Using a shader that darkens the edges of the view
    console.log(`Vignette: ${intensity.toFixed(2)}`)
  }

  /**
   * Get current position
   */
  getPosition(): THREE.Vector3 {
    return this.currentPosition.clone()
  }

  /**
   * Get current rotation
   */
  getRotation(): number {
    return this.currentRotation
  }
}

