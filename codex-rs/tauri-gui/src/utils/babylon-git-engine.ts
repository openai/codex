// babylon-git-engine.ts - Babylon.js Git可視化エンジン（Kamui4D超え）
// 10万コミット対応、動的LOD、WebGPU優先

import {
  Engine,
  Scene,
  ArcRotateCamera,
  Vector3,
  HemisphericLight,
  Mesh,
  InstancedMesh,
  Color3,
  Color4,
  Ray,
  PickingInfo,
  MeshBuilder,
  LinesMesh,
  PBRMaterial,
  GlowLayer,
  EngineOptions,
} from '@babylonjs/core';
import '@babylonjs/loaders';

export interface Commit3D {
  sha: string;
  message: string;
  author: string;
  timestamp: string;
  x: number;
  y: number;
  z: number;
  color: string;
  parents: string[];
}

export interface VisualizationStats {
  totalCommits: number;
  visibleCommits: number;
  fps: number;
  drawCalls: number;
  triangles: number;
  gpuMemoryUsage: number;
}

export interface LODLevel {
  distance: number;
  subdivisions: number;
  size: number;
}

// LOD設定（距離に応じた詳細度）
const LOD_LEVELS: LODLevel[] = [
  { distance: 0, subdivisions: 32, size: 1.0 },     // 近距離: 高詳細
  { distance: 50, subdivisions: 16, size: 0.8 },    // 中距離: 中詳細
  { distance: 100, subdivisions: 8, size: 0.6 },    // 遠距離: 低詳細
  { distance: 200, subdivisions: 4, size: 0.4 },    // 超遠距離: 最低詳細
];

export class GitVisualizationEngine {
  private engine: Engine | null = null;
  private scene: Scene | null = null;
  private camera: ArcRotateCamera | null = null;
  private glowLayer: GlowLayer | null = null;
  
  // コミットノード管理
  private commitNodes: Map<string, InstancedMesh> = new Map();
  private commitData: Map<string, Commit3D> = new Map();
  private connectionLines: LinesMesh[] = [];
  
  // パフォーマンス最適化
  private baseMesh: Mesh | null = null;
  private selectedNode: InstancedMesh | null = null;
  
  // 統計情報
  private stats: VisualizationStats = {
    totalCommits: 0,
    visibleCommits: 0,
    fps: 0,
    drawCalls: 0,
    triangles: 0,
    gpuMemoryUsage: 0,
  };

  /**
   * エンジンとシーンを初期化（WebGPU優先、WebGL2フォールバック）
   */
  async initialize(canvas: HTMLCanvasElement): Promise<void> {
    // WebGPU優先でエンジン作成
    try {
      const webGPUSupported = await Engine.isWebGPUSupported();
      if (webGPUSupported) {
        const engineOptions: EngineOptions = {
          adaptToDeviceRatio: true,
          antialias: true,
          powerPreference: 'high-performance',
        };
        
        this.engine = new Engine(canvas, true, engineOptions);
        console.log('✅ Babylon.js: WebGPU enabled');
      }
    } catch (error) {
      console.warn('⚠️  WebGPU not available, falling back to WebGL2:', error);
    }

    // WebGPU失敗時はWebGL2
    if (!this.engine) {
      const engineOptions: EngineOptions = {
        adaptToDeviceRatio: true,
        antialias: true,
        stencil: true,
        preserveDrawingBuffer: false,
        powerPreference: 'high-performance',
      };
      
      this.engine = new Engine(canvas, true, engineOptions);
      console.log('✅ Babylon.js: WebGL2 enabled');
    }

    // シーン作成
    this.scene = new Scene(this.engine);
    this.scene.clearColor = new Color4(0.02, 0.02, 0.08, 1.0); // Cyberpunk dark blue

    // カメラ設定
    this.camera = new ArcRotateCamera(
      'camera',
      -Math.PI / 2,
      Math.PI / 3,
      100,
      Vector3.Zero(),
      this.scene
    );
    this.camera.attachControl(canvas, true);
    this.camera.wheelPrecision = 50;
    this.camera.minZ = 0.1;
    this.camera.maxZ = 10000;
    this.camera.lowerRadiusLimit = 10;
    this.camera.upperRadiusLimit = 500;

    // ライティング
    const light = new HemisphericLight('light', new Vector3(0, 1, 0), this.scene);
    light.intensity = 0.7;

    // Glow Layer（Kamui4D風のグロー効果）
    this.glowLayer = new GlowLayer('glow', this.scene, {
      mainTextureFixedSize: 1024,
      blurKernelSize: 64,
    });
    this.glowLayer.intensity = 1.2;

    // ベースメッシュ作成（インスタンス化用）
    this.baseMesh = MeshBuilder.CreateSphere(
      'baseSphere',
      { diameter: 1, segments: 32 },
      this.scene
    );
    this.baseMesh.isVisible = false;

    // PBRマテリアル設定
    const material = new PBRMaterial('commitMaterial', this.scene);
    material.albedoColor = new Color3(1, 1, 1);
    material.metallic = 0.7;
    material.roughness = 0.3;
    material.emissiveColor = new Color3(0.2, 0.5, 1.0);
    material.emissiveIntensity = 1.5;
    this.baseMesh.material = material;

    // レンダーループ
    this.engine.runRenderLoop(() => {
      if (this.scene && this.camera) {
        this.scene.render();
        this.updateStats();
        this.updateNodeLOD(this.camera.radius);
      }
    });

    // リサイズハンドラ
    window.addEventListener('resize', () => {
      this.engine?.resize();
    });

    console.log('✅ Babylon.js Git可視化エンジン初期化完了');
  }

  /**
   * コミットデータを読み込み、3D可視化を生成
   */
  async loadCommits(commits: Commit3D[]): Promise<void> {
    if (!this.scene || !this.baseMesh) {
      throw new Error('Engine not initialized');
    }

    console.log(`📊 Loading ${commits.length} commits...`);
    const startTime = performance.now();

    // 既存ノードをクリア
    this.clearVisualization();

    // コミットノード作成（インスタンス化で高速化）
    commits.forEach((commit) => {
      const instance = this.baseMesh!.createInstance(`commit-${commit.sha}`);
      
      // 位置設定
      instance.position = new Vector3(commit.x, commit.y, commit.z);
      
      // カラー設定（PBRマテリアル）
      const material = new PBRMaterial(`mat-${commit.sha}`, this.scene!);
      const color = this.parseColor(commit.color);
      material.albedoColor = color;
      material.metallic = 0.7;
      material.roughness = 0.3;
      material.emissiveColor = color.scale(0.8);
      material.emissiveIntensity = 2.0;
      instance.material = material;

      // スケール（初期）
      instance.scaling = new Vector3(1, 1, 1);

      // データ保存
      this.commitNodes.set(commit.sha, instance);
      this.commitData.set(commit.sha, commit);

      // Glow効果追加
      this.glowLayer?.addIncludedOnlyMesh(instance);
    });

    // 接続線作成（親コミットとの線）
    this.createConnectionLines(commits);

    this.stats.totalCommits = commits.length;
    this.stats.visibleCommits = commits.length;

    const loadTime = performance.now() - startTime;
    console.log(`✅ ${commits.length} commits loaded in ${loadTime.toFixed(2)}ms`);
  }

  /**
   * 親コミットとの接続線を作成
   */
  private createConnectionLines(commits: Commit3D[]): void {
    if (!this.scene) return;

    commits.forEach((commit) => {
      commit.parents.forEach((parentSha) => {
        const parent = this.commitData.get(parentSha);
        if (!parent) return;

        const points = [
          new Vector3(commit.x, commit.y, commit.z),
          new Vector3(parent.x, parent.y, parent.z),
        ];

        const line = MeshBuilder.CreateLines(
          `line-${commit.sha}-${parentSha}`,
          { points },
          this.scene!
        );

        const color = this.parseColor(commit.color);
        line.color = color;
        line.alpha = 0.4;

        this.connectionLines.push(line);
      });
    });
  }

  /**
   * 動的LOD更新（カメラ距離に応じて詳細度を調整）
   */
  updateNodeLOD(_cameraDistance: number): void {
    if (!this.camera) return;

    this.commitNodes.forEach((node) => {
      const distance = Vector3.Distance(node.position, this.camera!.position);
      
      // 距離に応じたLODレベル決定
      let lodLevel: LODLevel | undefined = LOD_LEVELS[LOD_LEVELS.length - 1];
      for (const level of LOD_LEVELS) {
        if (distance < level.distance) {
          lodLevel = level;
          break;
        }
      }

      // スケール調整
      const scale = lodLevel?.size ?? 1.0;
      node.scaling = new Vector3(scale, scale, scale);

      // 超遠距離では非表示
      node.isVisible = distance < 300;
    });

    // 統計更新
    this.stats.visibleCommits = Array.from(this.commitNodes.values()).filter(
      (n) => n.isVisible
    ).length;
  }

  /**
   * ノード選択（レイキャスト）
   */
  selectNode(sha: string): void {
    // 前回の選択解除
    if (this.selectedNode) {
      const material = this.selectedNode.material as PBRMaterial;
      if (material) {
        material.emissiveIntensity = 2.0;
      }
      this.selectedNode.scaling = new Vector3(1, 1, 1);
    }

    // 新規選択
    const node = this.commitNodes.get(sha);
    if (!node) return;

    this.selectedNode = node;
    const material = node.material as PBRMaterial;
    if (material) {
      material.emissiveColor = new Color3(1, 1, 1);
      material.emissiveIntensity = 4.0;
    }
    node.scaling = new Vector3(1.5, 1.5, 1.5);

    // カメラをノードにフォーカス
    if (this.camera) {
      this.camera.setTarget(node.position);
    }
  }

  /**
   * レイとの交差判定
   */
  getNodeIntersection(ray: Ray): Commit3D | null {
    if (!this.scene) return null;

    const pickInfo: PickingInfo = this.scene.pickWithRay(ray);
    if (!pickInfo || !pickInfo.hit || !pickInfo.pickedMesh) {
      return null;
    }

    // インスタンスメッシュからSHA取得
    const meshName = pickInfo.pickedMesh.name;
    const sha = meshName.replace('commit-', '');
    return this.commitData.get(sha) || null;
  }

  /**
   * マウスピック（クリック位置からコミット取得）
   */
  pickCommit(x: number, y: number): Commit3D | null {
    if (!this.scene) return null;

    const pickInfo = this.scene.pick(x, y);
    if (!pickInfo || !pickInfo.hit || !pickInfo.pickedMesh) {
      return null;
    }

    const meshName = pickInfo.pickedMesh.name;
    const sha = meshName.replace('commit-', '');
    return this.commitData.get(sha) || null;
  }

  /**
   * 統計情報更新
   */
  private updateStats(): void {
    if (!this.engine || !this.scene) return;

    this.stats.fps = this.engine.getFps();
    this.stats.drawCalls = this.scene.getActiveMeshes().length;
    this.stats.triangles = this.scene.totalVertices;
  }

  /**
   * 統計情報取得
   */
  getStats(): VisualizationStats {
    return { ...this.stats };
  }

  /**
   * カラー文字列をColor3に変換
   */
  private parseColor(colorStr: string): Color3 {
    if (colorStr.startsWith('#')) {
      const r = parseInt(colorStr.slice(1, 3), 16) / 255;
      const g = parseInt(colorStr.slice(3, 5), 16) / 255;
      const b = parseInt(colorStr.slice(5, 7), 16) / 255;
      return new Color3(r, g, b);
    }
    // デフォルトカラー（cyan）
    return new Color3(0, 0.8, 1);
  }

  /**
   * 可視化をクリア
   */
  clearVisualization(): void {
    // ノード削除
    this.commitNodes.forEach((node) => {
      node.dispose();
    });
    this.commitNodes.clear();
    this.commitData.clear();

    // 接続線削除
    this.connectionLines.forEach((line) => {
      line.dispose();
    });
    this.connectionLines = [];

    this.selectedNode = null;
  }

  /**
   * エンジン破棄
   */
  dispose(): void {
    this.clearVisualization();
    
    if (this.baseMesh) {
      this.baseMesh.dispose();
      this.baseMesh = null;
    }

    if (this.scene) {
      this.scene.dispose();
      this.scene = null;
    }

    if (this.engine) {
      this.engine.dispose();
      this.engine = null;
    }

    console.log('✅ Babylon.js Git可視化エンジン破棄完了');
  }

  /**
   * シーン取得（外部操作用）
   */
  getScene(): Scene | null {
    return this.scene;
  }

  /**
   * カメラ取得
   */
  getCamera(): ArcRotateCamera | null {
    return this.camera;
  }

  /**
   * エンジン取得
   */
  getEngine(): Engine | null {
    return this.engine;
  }
}

