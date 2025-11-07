// BabylonARScene.tsx - WebXR AR統合（Kamui4D超え）
// 平面検出、ハンドジェスチャー、現実空間Git可視化

import { useEffect, useRef, useState } from 'react';
import {
  Engine,
  Scene,
  FreeCamera,
  Vector3,
  HemisphericLight,
  WebXRDefaultExperience,
  WebXRState,
  WebXRFeatureName,
  WebXRHitTest,
  WebXRPlaneDetector,
  MeshBuilder,
  StandardMaterial,
  Color3,
  Mesh,
} from '@babylonjs/core';
import '@babylonjs/loaders';
import type { Commit3D } from '../../utils/babylon-git-engine';
import '../../styles/BabylonARScene.css';

export interface BabylonARSceneProps {
  commits: Commit3D[];
  onCommitClick?: (commit: Commit3D) => void;
}

export default function BabylonARScene({ commits, onCommitClick }: BabylonARSceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<Engine | null>(null);
  const sceneRef = useRef<Scene | null>(null);
  const xrHelperRef = useRef<WebXRDefaultExperience | null>(null);
  
  const [arSupported, setArSupported] = useState<boolean>(false);
  const [arActive, setArActive] = useState<boolean>(false);
  const [planesDetected, setPlanesDetected] = useState<number>(0);
  const [anchorPlaced, setAnchorPlaced] = useState<boolean>(false);

  useEffect(() => {
    if (!canvasRef.current) return;

    const initAR = async () => {
      // エンジン作成
      const engine = new Engine(canvasRef.current!, true, {
        adaptToDeviceRatio: true,
        antialias: true,
      });
      engineRef.current = engine;

      // シーン作成
      const scene = new Scene(engine);
      scene.clearColor = new Color3(0, 0, 0).toColor4(0); // 透明（ARカメラ透過）
      sceneRef.current = scene;

      // カメラ（AR開始前のプレビュー用）
      const camera = new FreeCamera('camera', new Vector3(0, 1.6, -5), scene);
      camera.attachControl(canvasRef.current!, true);

      // ライト
      const light = new HemisphericLight('light', new Vector3(0, 1, 0), scene);
      light.intensity = 1.0;

      // WebXR AR対応確認
      try {
        const xrHelper = await WebXRDefaultExperience.CreateAsync(scene, {
          uiOptions: {
            sessionMode: 'immersive-ar',
          },
          optionalFeatures: true,
        });

        xrHelperRef.current = xrHelper;
        setArSupported(true);
        console.log('✅ WebXR AR supported');

        // AR状態変更
        xrHelper.baseExperience.onStateChangedObservable.add((state) => {
          if (state === WebXRState.IN_XR) {
            setArActive(true);
            console.log('📱 AR session started');
          } else if (state === WebXRState.NOT_IN_XR) {
            setArActive(false);
            console.log('📱 AR session ended');
          }
        });

        // 平面検出
        const planeDetector = xrHelper.baseExperience.featuresManager.enableFeature(
          WebXRFeatureName.PLANE_DETECTION,
          'latest',
          {}
        ) as WebXRPlaneDetector | null;

        if (planeDetector) {
          planeDetector.onPlaneAddedObservable.add((plane) => {
            console.log('📐 Plane detected:', plane.id);
            setPlanesDetected((prev) => prev + 1);
            
            // 平面メッシュ作成（デバッグ用）
            const planeMesh = MeshBuilder.CreatePlane(
              `plane-${plane.id}`,
              { size: 1 },
              scene
            );
            planeMesh.rotationQuaternion = plane.rotationQuaternion;
            planeMesh.position = plane.position;
            
            const material = new StandardMaterial(`plane-mat-${plane.id}`, scene);
            material.alpha = 0.3;
            material.emissiveColor = new Color3(0, 1, 0);
            planeMesh.material = material;
          });
        }

        // ヒットテスト（タップ位置にオブジェクト配置）
        const hitTest = xrHelper.baseExperience.featuresManager.enableFeature(
          WebXRFeatureName.HIT_TEST,
          'latest',
          {}
        ) as WebXRHitTest | null;

        if (hitTest) {
          hitTest.onHitTestResultObservable.add((results) => {
            if (results.length > 0 && !anchorPlaced) {
              const result = results[0];
              
              // コミット可視化を配置
              placeCommitVisualization(scene, result.position, commits);
              setAnchorPlaced(true);
            }
          });
        }

      } catch (error) {
        console.error('❌ WebXR AR not supported:', error);
        setArSupported(false);
      }

      // レンダーループ
      engine.runRenderLoop(() => {
        scene.render();
      });

      // リサイズ
      window.addEventListener('resize', () => {
        engine.resize();
      });
    };

    initAR();

    return () => {
      if (sceneRef.current) {
        sceneRef.current.dispose();
      }
      if (engineRef.current) {
        engineRef.current.dispose();
      }
    };
  }, [commits, anchorPlaced]);

  /**
   * ARセッション開始
   */
  const enterAR = async () => {
    if (!xrHelperRef.current) {
      console.error('❌ WebXR AR not initialized');
      return;
    }

    try {
      await xrHelperRef.current.baseExperience.enterXRAsync('immersive-ar', 'unbounded');
      console.log('✅ Entered AR mode');
    } catch (error) {
      console.error('❌ Failed to enter AR:', error);
    }
  };

  return (
    <div className="babylon-ar-scene-container">
      <canvas ref={canvasRef} className="babylon-ar-canvas" />

      {!arActive && (
        <div className="ar-controls">
          <h3>WebXR AR Mode</h3>
          {arSupported ? (
            <div>
              <button className="btn-enter-ar" onClick={enterAR}>
                📱 Enter AR
              </button>
              <p className="ar-hint">Tap to place Git visualization in real space</p>
            </div>
          ) : (
            <p className="ar-not-supported">
              ⚠️ WebXR AR not supported on this device
            </p>
          )}
        </div>
      )}

      {arActive && (
        <div className="ar-active-overlay">
          <p>AR Session Active</p>
          <p className="ar-stats">Planes detected: {planesDetected}</p>
          {!anchorPlaced && (
            <p className="ar-instruction">👆 Tap to place visualization</p>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * AR空間にコミット可視化を配置
 */
function placeCommitVisualization(scene: Scene, position: Vector3, commits: Commit3D[]): void {
  console.log('📍 Placing commit visualization at:', position);

  // コミット群を縮小して配置
  commits.slice(0, 50).forEach((commit, index) => {
    const sphere = MeshBuilder.CreateSphere(
      `ar-commit-${commit.sha}`,
      { diameter: 0.05, segments: 8 },
      scene
    );

    // AR空間座標（配置位置からの相対座標）
    sphere.position = new Vector3(
      position.x + commit.x * 0.01,
      position.y + commit.y * 0.01,
      position.z + commit.z * 0.01
    );

    const material = new StandardMaterial(`ar-mat-${commit.sha}`, scene);
    const color = Color3.FromHexString(commit.color);
    material.emissiveColor = color;
    material.alpha = 0.9;
    sphere.material = material;
  });
}

























