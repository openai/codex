// VirtualDesktop Optimizer
// Quest Link / Air Link / VirtualDesktop streaming optimization

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
  
  constructor() {
    this.currentPreset = VD_PRESETS.medium
    this.detectVirtualDesktop()
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

// CYBERPUNK_COLORS定義（ARScene内で使用）
const CYBERPUNK_COLORS = [
  '#00d4ff',
  '#b84fff',
  '#ff006e',
  '#39ff14',
  '#ffff00',
  '#ff3131',
  '#00ffff',
  '#ff00ff',
]

