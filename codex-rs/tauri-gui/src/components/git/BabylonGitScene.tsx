// BabylonGitScene.tsx - Babylon.js Git可視化コンポーネント（Kamui4D超え）
// React統合、10万コミット対応、動的LOD、PBRマテリアル

import { useEffect, useRef, useState, useCallback } from 'react';
import { GitVisualizationEngine, Commit3D, VisualizationStats } from '../../utils/babylon-git-engine';
import '../../styles/BabylonGitScene.css';

export interface BabylonGitSceneProps {
  commits: Commit3D[];
  onCommitClick?: (commit: Commit3D) => void;
  selectedCommitSha?: string;
  showStats?: boolean;
  showMinimap?: boolean;
}

export default function BabylonGitScene({
  commits,
  onCommitClick,
  selectedCommitSha,
  showStats = true,
  showMinimap = false,
}: BabylonGitSceneProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const engineRef = useRef<GitVisualizationEngine | null>(null);
  const [stats, setStats] = useState<VisualizationStats>({
    totalCommits: 0,
    visibleCommits: 0,
    fps: 0,
    drawCalls: 0,
    triangles: 0,
    gpuMemoryUsage: 0,
  });
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [selectedCommit, setSelectedCommit] = useState<Commit3D | null>(null);

  // エンジン初期化
  useEffect(() => {
    if (!canvasRef.current) return;

    const initEngine = async () => {
      setIsLoading(true);
      try {
        const engine = new GitVisualizationEngine();
        await engine.initialize(canvasRef.current!);
        engineRef.current = engine;
        
        console.log('✅ BabylonGitScene initialized');
        setIsLoading(false);
      } catch (error) {
        console.error('❌ Failed to initialize BabylonGitScene:', error);
        setIsLoading(false);
      }
    };

    initEngine();

    // クリーンアップ
    return () => {
      if (engineRef.current) {
        engineRef.current.dispose();
        engineRef.current = null;
      }
    };
  }, []);

  // コミットデータ読み込み
  useEffect(() => {
    if (!engineRef.current || commits.length === 0) return;

    const loadData = async () => {
      setIsLoading(true);
      try {
        await engineRef.current!.loadCommits(commits);
        console.log(`✅ Loaded ${commits.length} commits`);
        setIsLoading(false);
      } catch (error) {
        console.error('❌ Failed to load commits:', error);
        setIsLoading(false);
      }
    };

    loadData();
  }, [commits]);

  // 選択コミット更新
  useEffect(() => {
    if (!engineRef.current || !selectedCommitSha) return;
    engineRef.current.selectNode(selectedCommitSha);
  }, [selectedCommitSha]);

  // 統計情報更新（60fps）
  useEffect(() => {
    if (!showStats || !engineRef.current) return;

    const intervalId = setInterval(() => {
      if (engineRef.current) {
        const newStats = engineRef.current.getStats();
        setStats(newStats);
      }
    }, 1000 / 60);

    return () => clearInterval(intervalId);
  }, [showStats]);

  // キャンバスクリック処理
  const handleCanvasClick = useCallback((event: React.MouseEvent<HTMLCanvasElement>) => {
    if (!engineRef.current || !onCommitClick) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;

    const commit = engineRef.current.pickCommit(x, y);
    if (commit) {
      setSelectedCommit(commit);
      onCommitClick(commit);
      console.log('Selected commit:', commit.sha);
    }
  }, [onCommitClick]);

  // キーボードショートカット
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!engineRef.current) return;

      const camera = engineRef.current.getCamera();
      if (!camera) return;

      switch (event.key) {
        case 'r':
        case 'R':
          // リセット
          camera.alpha = -Math.PI / 2;
          camera.beta = Math.PI / 3;
          camera.radius = 100;
          break;
        case 'f':
        case 'F':
          // フルスクリーン切り替え
          if (canvasRef.current) {
            if (!document.fullscreenElement) {
              canvasRef.current.requestFullscreen();
            } else {
              document.exitFullscreen();
            }
          }
          break;
        case 'i':
        case 'I':
          // インスペクター表示（デバッグ用）
          const scene = engineRef.current.getScene();
          if (scene) {
            // Babylon.js Inspectorを開く（開発時のみ）
            import('@babylonjs/inspector').then((Inspector) => {
              Inspector.Inspector.Show(scene, {});
            });
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div className="babylon-git-scene-container">
      {/* キャンバス */}
      <canvas
        ref={canvasRef}
        className="babylon-git-canvas"
        onClick={handleCanvasClick}
      />

      {/* ローディング表示 */}
      {isLoading && (
        <div className="loading-overlay">
          <div className="loading-spinner">
            <div className="spinner"></div>
            <p>Loading Git visualization...</p>
            <p className="loading-detail">
              {commits.length > 0 ? `${commits.length} commits` : 'Initializing engine'}
            </p>
          </div>
        </div>
      )}

      {/* 統計パネル */}
      {showStats && !isLoading && (
        <div className="stats-panel">
          <h3>Performance</h3>
          <div className="stat-row">
            <span className="stat-label">FPS:</span>
            <span className={`stat-value ${stats.fps < 30 ? 'warning' : ''}`}>
              {stats.fps.toFixed(0)}
            </span>
          </div>
          <div className="stat-row">
            <span className="stat-label">Commits:</span>
            <span className="stat-value">
              {stats.visibleCommits} / {stats.totalCommits}
            </span>
          </div>
          <div className="stat-row">
            <span className="stat-label">Draw Calls:</span>
            <span className="stat-value">{stats.drawCalls}</span>
          </div>
          <div className="stat-row">
            <span className="stat-label">Triangles:</span>
            <span className="stat-value">
              {(stats.triangles / 1000).toFixed(1)}K
            </span>
          </div>
        </div>
      )}

      {/* 選択コミット情報 */}
      {selectedCommit && (
        <div className="selected-commit-panel">
          <h3>Selected Commit</h3>
          <div className="commit-info">
            <p className="commit-sha">{selectedCommit.sha.substring(0, 8)}</p>
            <p className="commit-message">{selectedCommit.message}</p>
            <p className="commit-author">{selectedCommit.author}</p>
            <p className="commit-timestamp">
              {new Date(selectedCommit.timestamp).toLocaleString()}
            </p>
          </div>
        </div>
      )}

      {/* コントロールヘルプ */}
      <div className="controls-help">
        <p><kbd>R</kbd> Reset view</p>
        <p><kbd>F</kbd> Fullscreen</p>
        <p><kbd>I</kbd> Inspector (dev)</p>
        <p><kbd>Mouse Drag</kbd> Rotate</p>
        <p><kbd>Mouse Wheel</kbd> Zoom</p>
      </div>

      {/* ミニマップ（オプション） */}
      {showMinimap && (
        <div className="minimap-container">
          <canvas className="minimap-canvas" />
        </div>
      )}
    </div>
  );
}

























