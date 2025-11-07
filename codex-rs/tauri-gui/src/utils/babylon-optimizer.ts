// babylon-optimizer.ts - Babylon.js パフォーマンス最適化（Kamui4D超え）
// GPU統計、動的品質調整、Virtual Desktop対応、CUDA統合

import { Engine, Scene } from '@babylonjs/core';
import { invoke } from '@tauri-apps/api/core';

export interface GPUStats {
  utilization: number;
  memoryUsed: number;
  memoryTotal: number;
  temperature: number;
  powerDraw: number;
  fanSpeed: number;
}

export interface QualityProfile {
  name: string;
  shadowQuality: number;
  antialiasing: boolean;
  postProcessing: boolean;
  glowIntensity: number;
  maxVisibleNodes: number;
  lodDistance: number[];
}

export interface OptimizationMetrics {
  fps: number;
  frameTime: number;
  gpuUtilization: number;
  cpuUtilization: number;
  memoryUsage: number;
  drawCalls: number;
  triangles: number;
  currentQuality: string;
}

// 品質プロファイル定義
const QUALITY_PROFILES: Record<string, QualityProfile> = {
  ultra: {
    name: 'Ultra',
    shadowQuality: 2,
    antialiasing: true,
    postProcessing: true,
    glowIntensity: 1.5,
    maxVisibleNodes: 100000,
    lodDistance: [0, 50, 100, 200],
  },
  high: {
    name: 'High',
    shadowQuality: 1,
    antialiasing: true,
    postProcessing: true,
    glowIntensity: 1.2,
    maxVisibleNodes: 50000,
    lodDistance: [0, 40, 80, 150],
  },
  medium: {
    name: 'Medium',
    shadowQuality: 1,
    antialiasing: false,
    postProcessing: true,
    glowIntensity: 0.8,
    maxVisibleNodes: 25000,
    lodDistance: [0, 30, 60, 120],
  },
  low: {
    name: 'Low',
    shadowQuality: 0,
    antialiasing: false,
    postProcessing: false,
    glowIntensity: 0.5,
    maxVisibleNodes: 10000,
    lodDistance: [0, 20, 40, 80],
  },
  potato: {
    name: 'Potato',
    shadowQuality: 0,
    antialiasing: false,
    postProcessing: false,
    glowIntensity: 0.2,
    maxVisibleNodes: 5000,
    lodDistance: [0, 15, 30, 60],
  },
};

// FPS閾値
const FPS_THRESHOLDS = {
  HIGH: 90,
  TARGET: 60,
  LOW: 30,
  CRITICAL: 15,
};

export class BabylonOptimizer {
  private engine: Engine | null = null;
  private scene: Scene | null = null;
  
  private currentQuality: QualityProfile = QUALITY_PROFILES['high'] ?? QUALITY_PROFILES['medium'];
  private autoQualityEnabled: boolean = true;
  private cudaEnabled: boolean = false;
  
  private frameTimeHistory: number[] = [];
  private readonly FRAME_HISTORY_SIZE = 60; // 1秒分のフレーム時間
  
  private gpuStats: GPUStats = {
    utilization: 0,
    memoryUsed: 0,
    memoryTotal: 0,
    temperature: 0,
    powerDraw: 0,
    fanSpeed: 0,
  };

  private isVirtualDesktop: boolean = false;
  private dpiScale: number = 1.0;

  /**
   * 初期化
   */
  async initialize(engine: Engine, scene: Scene): Promise<void> {
    this.engine = engine;
    this.scene = scene;

    // Virtual Desktop検出
    await this.detectVirtualDesktop();

    // CUDA利用可能確認
    await this.checkCudaAvailability();

    // 初期品質設定
    await this.detectOptimalQuality();

    console.log('✅ Babylon Optimizer initialized');
    console.log(`   Quality: ${this.currentQuality.name}`);
    console.log(`   Virtual Desktop: ${this.isVirtualDesktop}`);
    console.log(`   CUDA: ${this.cudaEnabled ? 'Enabled' : 'Disabled'}`);
  }

  /**
   * Virtual Desktop検出（Tauri API経由）
   */
  private async detectVirtualDesktop(): Promise<void> {
    try {
      // Tauri window APIでモニター情報取得
      const monitors = await invoke<any>('get_monitor_info');
      
      // Virtual Desktop判定（DPI、解像度、複数モニター）
      if (monitors) {
        this.dpiScale = monitors.scaleFactor || 1.0;
        
        // DPI > 1.5 または 4K以上でVirtual Desktop想定
        if (this.dpiScale >= 1.5 || monitors.width >= 3840) {
          this.isVirtualDesktop = true;
          console.log(`🖥️  Virtual Desktop detected (DPI: ${this.dpiScale}x)`);
        }
      }
    } catch (error) {
      console.warn('⚠️  Failed to detect Virtual Desktop:', error);
    }
  }

  /**
   * CUDA利用可能確認
   */
  private async checkCudaAvailability(): Promise<void> {
    try {
      const available = await invoke<boolean>('is_cuda_available');
      this.cudaEnabled = available;
      
      if (this.cudaEnabled) {
        console.log('🚀 CUDA acceleration enabled');
      }
    } catch (error) {
      console.warn('⚠️  CUDA not available:', error);
      this.cudaEnabled = false;
    }
  }

  /**
   * 最適品質自動検出（GPU性能ベース）
   */
  private async detectOptimalQuality(): Promise<void> {
    try {
      // GPU統計取得
      await this.updateGPUStats();

      // GPUメモリ量で品質判定
      const totalMemoryGB = this.gpuStats.memoryTotal / (1024 * 1024 * 1024);
      
      if (totalMemoryGB >= 10) {
        this.setQuality('ultra');
      } else if (totalMemoryGB >= 6) {
        this.setQuality('high');
      } else if (totalMemoryGB >= 4) {
        this.setQuality('medium');
      } else if (totalMemoryGB >= 2) {
        this.setQuality('low');
      } else {
        this.setQuality('potato');
      }

      console.log(`🎮 Detected GPU memory: ${totalMemoryGB.toFixed(1)}GB`);
      console.log(`   Auto quality: ${this.currentQuality.name}`);
    } catch (error) {
      console.warn('⚠️  Failed to detect optimal quality:', error);
      this.setQuality('medium'); // フォールバック
    }
  }

  /**
   * GPU統計更新（Tauri IPC経由）
   */
  async updateGPUStats(): Promise<void> {
    try {
      const stats = await invoke<GPUStats>('get_gpu_stats');
      this.gpuStats = stats;
    } catch (error) {
      // GPU統計取得失敗時はダミーデータ
      console.warn('⚠️  Failed to get GPU stats:', error);
    }
  }

  /**
   * フレーム終了時の最適化処理（毎フレーム呼び出し）
   */
  onFrameEnd(): void {
    if (!this.engine || !this.autoQualityEnabled) return;

    // フレーム時間記録
    const frameTime = this.engine.getDeltaTime();
    this.frameTimeHistory.push(frameTime);
    if (this.frameTimeHistory.length > this.FRAME_HISTORY_SIZE) {
      this.frameTimeHistory.shift();
    }

    // 平均FPS計算
    const avgFrameTime = this.frameTimeHistory.reduce((a, b) => a + b, 0) / this.frameTimeHistory.length;
    const avgFps = 1000 / avgFrameTime;

    // 動的品質調整
    if (avgFps < FPS_THRESHOLDS.CRITICAL) {
      this.downgradeQuality();
    } else if (avgFps < FPS_THRESHOLDS.LOW) {
      this.downgradeQuality();
    } else if (avgFps > FPS_THRESHOLDS.HIGH) {
      this.upgradeQuality();
    }
  }

  /**
   * 品質ダウングレード
   */
  private downgradeQuality(): void {
    const qualities = ['ultra', 'high', 'medium', 'low', 'potato'];
    const currentIndex = qualities.indexOf(this.getQualityName());
    
    if (currentIndex >= 0 && currentIndex < qualities.length - 1) {
      const newQuality = qualities[currentIndex + 1];
      if (newQuality) {
        this.setQuality(newQuality);
        console.log(`📉 Quality downgraded to ${newQuality}`);
      }
    }
  }

  /**
   * 品質アップグレード
   */
  private upgradeQuality(): void {
    const qualities = ['potato', 'low', 'medium', 'high', 'ultra'];
    const currentIndex = qualities.indexOf(this.getQualityName());
    
    if (currentIndex >= 0 && currentIndex < qualities.length - 1) {
      const newQuality = qualities[currentIndex + 1];
      if (newQuality) {
        this.setQuality(newQuality);
        console.log(`📈 Quality upgraded to ${newQuality}`);
      }
    }
  }

  /**
   * 品質設定適用
   */
  setQuality(qualityName: string): void {
    const profile = QUALITY_PROFILES[qualityName];
    if (!profile || !this.engine || !this.scene) return;

    this.currentQuality = profile;

    // エンジン設定適用
    if (this.engine) {
      // アンチエイリアス
      this.engine.setHardwareScalingLevel(profile.antialiasing ? 1.0 : 2.0);
    }

    // シーン設定適用
    if (this.scene) {
      // 影品質
      this.scene.shadowsEnabled = profile.shadowQuality > 0;
      
      // ポストプロセス
      this.scene.postProcessesEnabled = profile.postProcessing;
    }

    console.log(`✅ Quality set to ${profile.name}`);
  }

  /**
   * 自動品質調整ON/OFF
   */
  setAutoQuality(enabled: boolean): void {
    this.autoQualityEnabled = enabled;
    console.log(`Auto quality: ${enabled ? 'Enabled' : 'Disabled'}`);
  }

  /**
   * 最適化メトリクス取得
   */
  getMetrics(): OptimizationMetrics {
    if (!this.engine || !this.scene) {
      return {
        fps: 0,
        frameTime: 0,
        gpuUtilization: 0,
        cpuUtilization: 0,
        memoryUsage: 0,
        drawCalls: 0,
        triangles: 0,
        currentQuality: 'unknown',
      };
    }

    const avgFrameTime = this.frameTimeHistory.length > 0
      ? this.frameTimeHistory.reduce((a, b) => a + b, 0) / this.frameTimeHistory.length
      : 0;
    const fps = avgFrameTime > 0 ? 1000 / avgFrameTime : 0;

    return {
      fps: Math.round(fps),
      frameTime: avgFrameTime,
      gpuUtilization: this.gpuStats.utilization,
      cpuUtilization: 0, // TODO: CPU統計実装
      memoryUsage: this.gpuStats.memoryUsed,
      drawCalls: this.scene.getActiveMeshes().length,
      triangles: this.scene.totalVertices,
      currentQuality: this.currentQuality.name,
    };
  }

  /**
   * GPU統計取得
   */
  getGPUStats(): GPUStats {
    return { ...this.gpuStats };
  }

  /**
   * 現在の品質名取得
   */
  getQualityName(): string {
    return this.currentQuality.name.toLowerCase();
  }

  /**
   * 現在の品質プロファイル取得
   */
  getQualityProfile(): QualityProfile {
    return { ...this.currentQuality };
  }

  /**
   * Virtual Desktop判定結果取得
   */
  isRunningOnVirtualDesktop(): boolean {
    return this.isVirtualDesktop;
  }

  /**
   * DPI倍率取得
   */
  getDPIScale(): number {
    return this.dpiScale;
  }

  /**
   * CUDA有効判定
   */
  isCudaEnabled(): boolean {
    return this.cudaEnabled;
  }

  /**
   * 利用可能な品質プロファイル一覧取得
   */
  static getAvailableProfiles(): string[] {
    return Object.keys(QUALITY_PROFILES);
  }

  /**
   * 品質プロファイル詳細取得
   */
  static getProfileDetails(qualityName: string): QualityProfile | null {
    return QUALITY_PROFILES[qualityName] || null;
  }
}

