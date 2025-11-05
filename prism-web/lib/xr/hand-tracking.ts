/**
 * Hand Tracking Integration for WebXR
 * Quest 3 Pro / Apple Vision Pro support
 */

import * as THREE from 'three'

export interface HandJoint {
  position: THREE.Vector3
  rotation: THREE.Quaternion
  radius: number
}

export interface HandPose {
  wrist: HandJoint
  thumb: HandJoint[]
  index: HandJoint[]
  middle: HandJoint[]
  ring: HandJoint[]
  pinky: HandJoint[]
}

/**
/**
 * Hand Tracking Manager
 */
export class HandTrackingManager {
  private leftHand: HandPose | null = null
  private rightHand: HandPose | null = null
  private supported: boolean = false

  constructor() {
    this.checkSupport()
  }

  private async checkSupport(): Promise<void> {
    if ('XRHand' in window) {
      this.supported = true
      console.log('✅ Hand tracking supported')
    } else {
      console.log('❌ Hand tracking not supported')
    }
  }

  /**
   * Update hand poses from XR session
   */
  update(session: XRSession, frame: XRFrame, referenceSpace: XRReferenceSpace): void {
    if (!this.supported) return

    // Get input sources
    for (const inputSource of session.inputSources) {
      if (inputSource.hand) {
        const hand = this.getHandPose(inputSource.hand, frame, referenceSpace)
        
        if (inputSource.handedness === 'left') {
          this.leftHand = hand
        } else if (inputSource.handedness === 'right') {
          this.rightHand = hand
        }
      }
    }
  }

  /**
   * Extract hand pose from XRHand
   */
  private getHandPose(hand: XRHand, frame: XRFrame, referenceSpace: XRReferenceSpace): HandPose | null {
    const getJoint = (jointName: XRHandJoint): HandJoint | null => {
      const joint = hand.get(jointName)
      if (!joint) return null

      const jointPose = frame.getJointPose(joint, referenceSpace)
      if (!jointPose) return null

      return {
        position: new THREE.Vector3(
          jointPose.transform.position.x,
          jointPose.transform.position.y,
          jointPose.transform.position.z
        ),
        rotation: new THREE.Quaternion(
          jointPose.transform.orientation.x,
          jointPose.transform.orientation.y,
          jointPose.transform.orientation.z,
          jointPose.transform.orientation.w
        ),
        radius: jointPose.radius || 0.01,
      }
    }

    const wrist = getJoint('wrist')
    if (!wrist) return null

    return {
      wrist,
      thumb: [
        getJoint('thumb-metacarpal'),
        getJoint('thumb-phalanx-proximal'),
        getJoint('thumb-phalanx-distal'),
        getJoint('thumb-tip'),
      ].filter(Boolean) as HandJoint[],
      index: [
        getJoint('index-finger-metacarpal'),
        getJoint('index-finger-phalanx-proximal'),
        getJoint('index-finger-phalanx-intermediate'),
        getJoint('index-finger-phalanx-distal'),
        getJoint('index-finger-tip'),
      ].filter(Boolean) as HandJoint[],
      middle: [
        getJoint('middle-finger-metacarpal'),
        getJoint('middle-finger-phalanx-proximal'),
        getJoint('middle-finger-phalanx-intermediate'),
        getJoint('middle-finger-phalanx-distal'),
        getJoint('middle-finger-tip'),
      ].filter(Boolean) as HandJoint[],
      ring: [
        getJoint('ring-finger-metacarpal'),
        getJoint('ring-finger-phalanx-proximal'),
        getJoint('ring-finger-phalanx-intermediate'),
        getJoint('ring-finger-phalanx-distal'),
        getJoint('ring-finger-tip'),
      ].filter(Boolean) as HandJoint[],
      pinky: [
        getJoint('pinky-finger-metacarpal'),
        getJoint('pinky-finger-phalanx-proximal'),
        getJoint('pinky-finger-phalanx-intermediate'),
        getJoint('pinky-finger-phalanx-distal'),
        getJoint('pinky-finger-tip'),
      ].filter(Boolean) as HandJoint[],
    }
  }

  /**
   * Detect pinch gesture (index finger + thumb)
   */
  isPinching(hand: 'left' | 'right'): boolean {
    const handPose = hand === 'left' ? this.leftHand : this.rightHand
    if (!handPose) return false

    const thumbTip = handPose.thumb[handPose.thumb.length - 1]
    const indexTip = handPose.index[handPose.index.length - 1]

    if (!thumbTip || !indexTip) return false

    const distance = thumbTip.position.distanceTo(indexTip.position)
    return distance < 0.02  // 2cm threshold
  }

  /**
   * Get pinch position (midpoint between thumb and index)
   */
  getPinchPosition(hand: 'left' | 'right'): THREE.Vector3 | null {
    const handPose = hand === 'left' ? this.leftHand : this.rightHand
    if (!handPose) return null

    const thumbTip = handPose.thumb[handPose.thumb.length - 1]
    const indexTip = handPose.index[handPose.index.length - 1]

    if (!thumbTip || !indexTip) return null

    return new THREE.Vector3()
      .addVectors(thumbTip.position, indexTip.position)
      .multiplyScalar(0.5)
  }

  /**
   * Detect pointing gesture (index extended, others curled)
   */
  isPointing(hand: 'left' | 'right'): boolean {
    const handPose = hand === 'left' ? this.leftHand : this.rightHand
    if (!handPose) return false

    // Check if index finger is extended
    const indexExtended = handPose.index.length >= 2 &&
      handPose.index[0].position.distanceTo(handPose.index[handPose.index.length - 1].position) > 0.08

    // Check if other fingers are curled
    const middleCurled = handPose.middle.length >= 2 &&
      handPose.middle[0].position.distanceTo(handPose.middle[handPose.middle.length - 1].position) < 0.06

    return indexExtended && middleCurled
  }

  /**
   * Get pointing direction
   */
  getPointingDirection(hand: 'left' | 'right'): THREE.Vector3 | null {
    const handPose = hand === 'left' ? this.leftHand : this.rightHand
    if (!handPose || !this.isPointing(hand)) return null

    const indexBase = handPose.index[0]
    const indexTip = handPose.index[handPose.index.length - 1]

    if (!indexBase || !indexTip) return null

    return new THREE.Vector3()
      .subVectors(indexTip.position, indexBase.position)
      .normalize()
  }

  getLeftHand(): HandPose | null {
    return this.leftHand
  }

  getRightHand(): HandPose | null {
    return this.rightHand
  }
}

export { HandTrackingManager }
export type { HandPose, HandJoint }

