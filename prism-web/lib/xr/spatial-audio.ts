/**
 * Spatial Audio for VR/AR
 * 3D positional audio for commit nodes
 */

import * as THREE from 'three'

export interface AudioConfig {
  enabled: boolean
  volume: number              // 0-1
  refDistance: number         // Distance where volume starts to decrease
  maxDistance: number         // Distance where volume reaches minimum
  rolloffFactor: number       // How quickly volume decreases
}

export class SpatialAudioManager {
  private listener: THREE.AudioListener | null = null
  private audioContext: AudioContext | null = null
  private sounds: Map<string, THREE.PositionalAudio> = new Map()
  private config: AudioConfig = {
    enabled: true,
    volume: 0.5,
    refDistance: 5,
    maxDistance: 50,
    rolloffFactor: 1,
  }

  constructor(camera: THREE.Camera, config?: Partial<AudioConfig>) {
    if (config) {
      this.config = { ...this.config, ...config }
    }

    this.initialize(camera)
  }

  /**
   * Initialize audio system
   */
  private initialize(camera: THREE.Camera): void {
    try {
      this.listener = new THREE.AudioListener()
      camera.add(this.listener)

      this.audioContext = this.listener.context

      console.log('笨・Spatial audio initialized')
    } catch (error) {
      console.error('笶・Failed to initialize spatial audio:', error)
    }
  }

  /**
   * Create positional audio for commit node
   */
  createCommitSound(
    commitSha: string,
    position: THREE.Vector3,
    audioUrl?: string
  ): THREE.PositionalAudio | null {
    if (!this.listener || !this.config.enabled) return null

    // Check if sound already exists
    if (this.sounds.has(commitSha)) {
      return this.sounds.get(commitSha)!
    }

    const sound = new THREE.PositionalAudio(this.listener)

    // Configure spatial parameters
    sound.setRefDistance(this.config.refDistance)
    sound.setMaxDistance(this.config.maxDistance)
    sound.setRolloffFactor(this.config.rolloffFactor)
    sound.setVolume(this.config.volume)

    // Load audio (would load from URL or generate procedurally)
    if (audioUrl) {
      const audioLoader = new THREE.AudioLoader()
      audioLoader.load(audioUrl, (buffer) => {
        sound.setBuffer(buffer)
      })
    } else {
      // Generate procedural sound based on commit properties
      this.generateProceduralSound(sound, commitSha)
    }

    // Position in 3D space
    const audioObject = new THREE.Object3D()
    audioObject.position.copy(position)
    audioObject.add(sound)

    this.sounds.set(commitSha, sound)

    return sound
  }

  /**
   * Generate procedural audio for commit
   * Different authors get different tones
   */
  private generateProceduralSound(sound: THREE.PositionalAudio, commitSha: string): void {
    if (!this.audioContext) return

    // Create oscillator for simple tone
    const duration = 0.5  // 500ms
    const frequency = this.getFrequencyFromSha(commitSha)

    const oscillator = this.audioContext.createOscillator()
    const gainNode = this.audioContext.createGain()

    oscillator.type = 'sine'
    oscillator.frequency.setValueAtTime(frequency, this.audioContext.currentTime)

    // Envelope
    gainNode.gain.setValueAtTime(0, this.audioContext.currentTime)
    gainNode.gain.linearRampToValueAtTime(this.config.volume, this.audioContext.currentTime + 0.01)
    gainNode.gain.exponentialRampToValueAtTime(0.01, this.audioContext.currentTime + duration)

    oscillator.connect(gainNode)
    // Note: Would connect to sound's audio node

    oscillator.start(this.audioContext.currentTime)
    oscillator.stop(this.audioContext.currentTime + duration)
  }

  /**
   * Map commit SHA to frequency (440-880 Hz range)
   */
  private getFrequencyFromSha(sha: string): number {
    const hash = sha.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0)
    return 440 + (hash % 440)  // A4 to A5
  }

  /**
   * Play sound at commit position
   */
  playCommitSound(commitSha: string, position: THREE.Vector3): void {
    const sound = this.sounds.get(commitSha)
    if (sound && !sound.isPlaying) {
      sound.play()
    } else {
      // Create and play new sound
      const newSound = this.createCommitSound(commitSha, position)
      if (newSound) {
        newSound.play()
      }
    }
  }

  /**
   * Play merge sound effect
   */
  playMergeSound(position: THREE.Vector3): void {
    if (!this.listener || !this.config.enabled) return

    const sound = new THREE.PositionalAudio(this.listener)
    sound.setRefDistance(this.config.refDistance)
    sound.setMaxDistance(this.config.maxDistance)

    // Load merge sound effect (whoosh or chime)
    // For now, use procedural sound
    console.log('矧 Merge sound at', position)
  }

  /**
   * Play branch creation sound
   */
  playBranchSound(position: THREE.Vector3): void {
    if (!this.listener || !this.config.enabled) return

    console.log('矧 Branch sound at', position)
  }

  /**
   * Update audio listener position (follow camera)
   */
  updateListener(camera: THREE.Camera): void {
    // Listener is attached to camera, automatically updated
  }

  /**
   * Enable/disable spatial audio
   */
  setEnabled(enabled: boolean): void {
    this.config.enabled = enabled

    if (!enabled) {
      // Stop all sounds
      this.sounds.forEach((sound) => {
        if (sound.isPlaying) {
          sound.stop()
        }
      })
    }
  }

  /**
   * Set master volume
   */
  setVolume(volume: number): void {
    this.config.volume = Math.max(0, Math.min(1, volume))

    // Update all existing sounds
    this.sounds.forEach((sound) => {
      sound.setVolume(this.config.volume)
    })
  }

  /**
   * Cleanup
   */
  dispose(): void {
    this.sounds.forEach((sound) => {
      if (sound.isPlaying) {
        sound.stop()
      }
      sound.disconnect()
    })

    this.sounds.clear()

    if (this.listener) {
      this.listener.clear()
    }

    console.log('Spatial audio disposed')
  }
}

