// BabylonVRScene.tsx - WebXR VR統合（Kamui4D超え）
// Quest 3対応、ハンドトラッキング、空間UI

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
  WebXRHandTracking,
  WebXRMotionControllerManager,
  MeshBuilder,
  StandardMaterial,
  Color3,
} from '@babylonjs/core';
import '@babylonjs/loaders';
import type { Commit3D } from '../../utils/babylon-git-engine';
import '../../styles/BabylonVRScene.css';

export interface BabylonVRSceneProps {
  commits: Commit3D[];
  onCommitClick?: (commit: Commit3D) => void;
}

export default function BabylonVRScene({ commits, onCommitClick }: BabylonVRSceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<Engine | null>(null);
  const sceneRef = useRef<Scene | null>(null);
  const xrHelperRef = useRef<WebXRDefaultExperience | null>(null);
  
  const [vrSupported, setVrSupported] = useState<boolean>(false);
  const [vrActive, setVrActive] = useState<boolean>(false);
  const [handTrackingActive, setHandTrackingActive] = useState<boolean>(false);

  useEffect(() => {
    if (!canvasRef.current) return;

    const initVR = async () => {
      // エンジン作成
      const engine = new Engine(canvasRef.current!, true, {
        adaptToDeviceRatio: true,
        antialias: true,
      });
      engineRef.current = engine;

      // シーン作成
      const scene = new Scene(engine);
      scene.clearColor = new Color3(0.02, 0.02, 0.08).toColor4(1);
      sceneRef.current = scene;

      // カメラ（VR開始前のデスクトップビュー用）
      const camera = new FreeCamera('camera', new Vector3(0, 1.6, -5), scene);
      camera.attachControl(canvasRef.current!, true);

      // ライト
      const light = new HemisphericLight('light', new Vector3(0, 1, 0), scene);
      light.intensity = 0.7;

      // WebXR対応確認
      const xrSupported = await WebXRDefaultExperience.CreateAsync(scene, {
        floorMeshes: [],
        disableTeleportation: false,
      });

      if (xrSupported) {
        xrHelperRef.current = xrSupported;
        setVrSupported(true);
        console.log('✅ WebXR supported');

        // VR状態変更イベント
        xrSupported.baseExperience.onStateChangedObservable.add((state) => {
          if (state === WebXRState.IN_XR) {
            setVrActive(true);
            console.log('🥽 VR session started');
          } else if (state === WebXRState.NOT_IN_XR) {
            setVrActive(false);
            console.log('🥽 VR session ended');
          }
        });

        // ハンドトラッキング（Quest 3対応）
        const handTracking = xrSupported.baseExperience.featuresManager.enableFeature(
          WebXRFeatureName.HAND_TRACKING,
          'latest',
          {}
        ) as WebXRHandTracking | null;

        if (handTracking) {
          setHandTrackingActive(true);
          console.log('👋 Hand tracking enabled');
        }

        // モーションコントローラー
        const controllers = xrSupported.input;
        controllers.onControllerAddedObservable.add((controller) => {
          console.log('🎮 Controller added:', controller.inputSource.handedness);
          
          // コントローラートリガー
          controller.onMotionControllerInitObservable.add((motionController) => {
            const trigger = motionController.getMainComponent();
            if (trigger) {
              trigger.onButtonStateChangedObservable.add((component) => {
                if (component.pressed) {
                  // トリガー押下時の処理
                  console.log('Trigger pressed');
                }
              });
            }
          });
        });
      } else {
        console.warn('⚠️  WebXR not supported');
      }

      // コミット可視化（簡易版）
      createCommitVisuals(scene, commits);

      // レンダーループ
      engine.runRenderLoop(() => {
        scene.render();
      });

      // リサイズ
      window.addEventListener('resize', () => {
        engine.resize();
      });
    };

    initVR();

    return () => {
      if (sceneRef.current) {
        sceneRef.current.dispose();
      }
      if (engineRef.current) {
        engineRef.current.dispose();
      }
    };
  }, []);

  /**
   * コミット可視化作成（VR空間用）
   */
  const createCommitVisuals = (scene: Scene, commits: Commit3D[]) => {
    commits.slice(0, 100).forEach((commit) => {
      const sphere = MeshBuilder.CreateSphere(
        `commit-${commit.sha}`,
        { diameter: 0.3, segments: 16 },
        scene
      );

      sphere.position = new Vector3(
        commit.x * 0.1,
        commit.y * 0.05 + 1.6,
        commit.z * 0.1
      );

      const material = new StandardMaterial(`mat-${commit.sha}`, scene);
      material.emissiveColor = Color3.FromHexString(commit.color);
      sphere.material = material;
    });
  };

  /**
   * VRセッション開始
   */
  const enterVR = async () => {
    if (!xrHelperRef.current) {
      console.error('❌ WebXR not initialized');
      return;
    }

    try {
      await xrHelperRef.current.baseExperience.enterXRAsync('immersive-vr', 'local-floor');
      console.log('✅ Entered VR mode');
    } catch (error) {
      console.error('❌ Failed to enter VR:', error);
    }
  };

  return (
    <div className="babylon-vr-scene-container">
      <canvas ref={canvasRef} className="babylon-vr-canvas" />

      {!vrActive && (
        <div className="vr-controls">
          <h3>WebXR VR Mode</h3>
          {vrSupported ? (
            <div>
              <button className="btn-enter-vr" onClick={enterVR}>
                🥽 Enter VR
              </button>
              {handTrackingActive && <p className="feature-badge">👋 Hand Tracking</p>}
            </div>
          ) : (
            <p className="vr-not-supported">
              ⚠️ WebXR not supported in this browser
            </p>
          )}
        </div>
      )}

      {vrActive && (
        <div className="vr-active-overlay">
          <p>VR Session Active</p>
          <p className="vr-hint">Look around and use controllers</p>
        </div>
      )}
    </div>
  );
}

























