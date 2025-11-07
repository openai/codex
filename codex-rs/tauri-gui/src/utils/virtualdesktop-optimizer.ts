// VirtualDesktop Optimizer
// Quest Link / Air Link / VirtualDesktop streaming optimization

import { useState, useEffect } from 'react';

export interface VDQualityPreset {
  name: string
  renderScale: number
  bloomIntensity: number
  chromaIntensity: number
  targetFps: number
  enablePostProcessing: boolean
  lodBias: number
}

export const VD_PRESETS: Record<string, VDQualityPreset> = {
  ultra: {
    name: 'Ultra (Local)',
    renderScale: 1.5,
    bloomIntensity: 2.0,
    chromaIntensity: 0.002,
    targetFps: 120,
    enablePostProcessing: true,
    lodBias: 0,
  },
  high: {
    name: 'High (WiFi 6)',
    renderScale: 1.2,
    bloomIntensity: 1.5,
    chromaIntensity: 0.0015,
    targetFps: 90,
    enablePostProcessing: true,
    lodBias: 0.5,
  },
  medium: {
    name: 'Medium (VirtualDesktop)',
    renderScale: 1.0,
    bloomIntensity: 1.0,
    chromaIntensity: 0.001,
    targetFps: 72,
    enablePostProcessing: true,
    lodBias: 1.0,
  },
  low: {
    name: 'Low (Mobile Hotspot)',
    renderScale: 0.8,
    bloomIntensity: 0.5,
    chromaIntensity: 0,
    targetFps: 60,
    enablePostProcessing: false,
    lodBias: 2.0,
  },
}

export class VirtualDesktopOptimizer {
  private currentPreset: VDQualityPreset
  private isVirtualDesktop: boolean = false
  private dpiScale: number = 1.0
  private vrHeadset: string | null = null
  
  constructor() {
    this.currentPreset = VD_PRESETS['medium'] ?? VD_PRESETS['low']
    this.detectVirtualDesktop()
    this.detectDPI()
    this.detectVRHeadset()
  }
  
  // VirtualDesktop検出
  detectVirtualDesktop(): boolean {
    // UserAgentからVD検出を試みる
    const ua = navigator.userAgent.toLowerCase()
    
    // VirtualDesktop特有のヘッダーやUA文字列
    this.isVirtualDesktop = 
      ua.includes('virtualdesktop') ||
      ua.includes('oculus') ||
      ua.includes('quest') ||
      this.checkVDConnection()
    
    if (this.isVirtualDesktop) {
      console.log('✓ VirtualDesktop detected - applying streaming optimizations')
      this.applyPreset('medium')
    }
    
    return this.isVirtualDesktop
  }
  
  // VD接続チェック（レイテンシベース）
  private checkVDConnection(): boolean {
    // Performance APIでネットワークレイテンシをチェック
    if (!window.performance || !window.performance.getEntriesByType) {
      return false
    }
    
    const navigation = window.performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming
    if (navigation) {
      const latency = navigation.responseStart - navigation.requestStart
      // 10ms以上のレイテンシ = ストリーミングの可能性
      return latency > 10
    }
    
    return false
  }
  
  // プリセット適用
  applyPreset(presetName: string): void {
    const preset = VD_PRESETS[presetName]
    if (preset) {
      this.currentPreset = preset
      console.log(`Applied preset: ${preset.name}`)
      
      // DOMにプリセット情報を保存（Reactコンポーネントから参照可能）
      document.documentElement.setAttribute('data-vd-preset', presetName)
      document.documentElement.setAttribute('data-vd-fps', preset.targetFps.toString())
      document.documentElement.setAttribute('data-vd-render-scale', preset.renderScale.toString())
    }
  }
  
  // ストリーミング最適化適用
  optimizeForStreaming(): void {
    // レンダリング解像度調整
    this.reduceRenderResolution()
    
    // ポストプロセス軽減
    this.reducePostProcessing()
    
    // LOD積極適用
    this.applyAggressiveLOD()
    
    // ネットワーク最適化
    this.reduceNetworkLoad()
  }
  
  private reduceRenderResolution(): void {
    const canvas = document.querySelector('canvas')
    if (canvas) {
      const scale = this.currentPreset.renderScale
      canvas.style.imageRendering = scale < 1 ? 'pixelated' : 'auto'
    }
  }
  
  private reducePostProcessing(): void {
    if (!this.currentPreset.enablePostProcessing) {
      document.documentElement.setAttribute('data-disable-postprocessing', 'true')
    }
  }
  
  private applyAggressiveLOD(): void {
    document.documentElement.setAttribute('data-lod-bias', this.currentPreset.lodBias.toString())
  }
  
  private reduceNetworkLoad(): void {
    // Delta updates only
    // Aggressive caching
    document.documentElement.setAttribute('data-cache-aggressive', 'true')
  }
  
  // FPS測定
  measureFPS(): number {
    let lastTime = performance.now()
    let frameCount = 0
    let fps = 0
    
    const measure = () => {
      frameCount++
      const currentTime = performance.now()
      const elapsed = currentTime - lastTime
      
      if (elapsed >= 1000) {
        fps = Math.round((frameCount * 1000) / elapsed)
        frameCount = 0
        lastTime = currentTime
      }
      
      requestAnimationFrame(measure)
    }
    
    measure()
    
    return fps
  }
  
  // 現在のプリセット取得
  getCurrentPreset(): VDQualityPreset {
    return this.currentPreset
  }
  
  // VirtualDesktop検出状態
  isUsingVirtualDesktop(): boolean {
    return this.isVirtualDesktop
  }

  // === Phase 3.2: DPI調整とVR Headset連携 ===

  /**
   * DPI検出と調整
   */
  detectDPI(): number {
    this.dpiScale = window.devicePixelRatio || 1.0
    
    if (this.dpiScale > 1.5) {
      console.log(`🖥️  High DPI detected: ${this.dpiScale}x`)
      // 高DPI環境ではレンダリング品質を調整
      if (this.dpiScale >= 2.0) {
        this.applyPreset('ultra')
      }
    }
    
    return this.dpiScale
  }

  /**
   * VR Headset検出（Quest Link / Air Link / Steam VR）
   */
  detectVRHeadset(): string | null {
    // WebXR API経由でVRデバイス検出
    if ('xr' in navigator) {
      const xr = (navigator as any).xr;
      if (xr && typeof xr.isSessionSupported === 'function') {
        xr.isSessionSupported('immersive-vr').then((supported: boolean) => {
        if (supported) {
          // VRデバイス情報取得（可能であれば）
          this.vrHeadset = 'WebXR Compatible Device'
          console.log('🥽 VR Headset detected')
          
          // Quest特有の最適化
          if (navigator.userAgent.includes('Quest')) {
            this.vrHeadset = 'Meta Quest'
            this.applyQuestOptimizations()
          }
        }
        }).catch((err: Error) => {
          console.warn('VR detection failed:', err)
        })
      }
    }
    
    return this.vrHeadset
  }

  /**
   * Quest専用最適化
   */
  private applyQuestOptimizations(): void {
    console.log('🥽 Applying Quest-specific optimizations')
    
    // Quest 3は高解像度だがモバイルGPU
    // 品質: High（Ultra不可）
    this.applyPreset('high')
    
    // フォビエイテッドレンダリング準備
    document.documentElement.setAttribute('data-foveated-rendering', 'true')
  }

  /**
   * DPI倍率取得
   */
  getDPIScale(): number {
    return this.dpiScale
  }

  /**
   * 検出されたVR Headset取得
   */
  getVRHeadset(): string | null {
    return this.vrHeadset
  }

  /**
   * VR Headset接続確認（リアルタイム）
   */
  async checkVRHeadsetConnection(): Promise<boolean> {
    if (!('xr' in navigator)) {
      return false
    }
    
    try {
      const xr = (navigator as any).xr;
      if (xr && typeof xr.isSessionSupported === 'function') {
        const supported = await xr.isSessionSupported('immersive-vr')
        return supported || false
      }
      return false
    } catch {
      return false
    }
  }

  /**
   * DPIスケール適用（Canvas解像度調整）
   */
  applyDPIScale(canvas: HTMLCanvasElement): void {
    const rect = canvas.getBoundingClientRect()
    canvas.width = rect.width * this.dpiScale
    canvas.height = rect.height * this.dpiScale
    console.log(`✅ Canvas resolution: ${canvas.width}x${canvas.height} (DPI: ${this.dpiScale}x)`)
  }
}

// Global singleton
export const vdOptimizer = new VirtualDesktopOptimizer()

// React Hook
export const useVirtualDesktopOptimizer = () => {
  const [preset, setPreset] = useState<VDQualityPreset>(vdOptimizer.getCurrentPreset())
  const [isVD, setIsVD] = useState(vdOptimizer.isUsingVirtualDesktop())
  
  const changePreset = (presetName: string) => {
    vdOptimizer.applyPreset(presetName)
    setPreset(vdOptimizer.getCurrentPreset())
  }
  
  useEffect(() => {
    const detected = vdOptimizer.detectVirtualDesktop()
    setIsVD(detected)
    
    if (detected) {
      vdOptimizer.optimizeForStreaming()
    }
  }, [])
  
  return {
    preset,
    isVD,
    changePreset,
    availablePresets: Object.keys(VD_PRESETS),
  }
}

