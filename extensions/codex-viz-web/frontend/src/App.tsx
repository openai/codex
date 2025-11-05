import { Canvas } from '@react-three/fiber'
import { OrbitControls, PerspectiveCamera } from '@react-three/drei'
import { Suspense, useState, useRef, useMemo, useEffect } from 'react'
import CommitGraph3D from './components/CommitGraph3D'
import FileHeatMap from './components/FileHeatMap'
import BranchStructure3D from './components/BranchStructure3D'
import RealtimeMonitor from './components/RealtimeMonitor'
import ControlPanel from './components/ControlPanel'
import LoadingScreen from './components/LoadingScreen'
import PerformanceMonitor from './components/PerformanceMonitor'
import KeyboardShortcutsHelp from './components/KeyboardShortcutsHelp'
import TimelineSlider from './components/TimelineSlider'
import SearchBar, { SearchFilters } from './components/SearchBar'
import BookmarksPanel from './components/BookmarksPanel'
import Toast, { ToastContainer, ToastType } from './components/Toast'
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts'
import { useBookmarks } from './hooks/useBookmarks'
import { useCommits } from './hooks/useGitData'
import './App.css'

type ViewMode = 'commits' | 'heatmap' | 'branches' | 'all'

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>('commits')
  const [repoPath, setRepoPath] = useState<string>('')
  const [showStats, setShowStats] = useState(false)
  const [showHelp, setShowHelp] = useState(false)
  const [showRealtime, setShowRealtime] = useState(true)
  const [showBookmarks, setShowBookmarks] = useState(false)
  const [currentDate, setCurrentDate] = useState<number>(Date.now())
  
  // Listen for bookmarks toggle event
  useEffect(() => {
    const handler = () => setShowBookmarks(prev => !prev)
    window.addEventListener('toggle-bookmarks', handler)
    return () => window.removeEventListener('toggle-bookmarks', handler)
  }, [])
  const [searchFilters, setSearchFilters] = useState<SearchFilters>({
    searchText: '',
    authors: [],
    branches: [],
    dateFrom: '',
    dateTo: '',
  })
  const [toasts, setToasts] = useState<Array<{ id: string; message: string; type: ToastType }>>([])
  
  const controlsRef = useRef<any>(null)
  const { bookmarks, addBookmark, isBookmarked } = useBookmarks()
  
  // Get commits for filters
  const { data: commits } = useCommits(repoPath)
  
  // Extract unique authors and branches
  const availableAuthors = useMemo(() => {
    if (!commits) return []
    return Array.from(new Set(commits.map(c => c.author_email)))
  }, [commits])
  
  const availableBranches = useMemo(() => {
    if (!commits) return []
    return Array.from(new Set(commits.map(c => c.branch)))
  }, [commits])
  
  // Calculate date range
  const { minDate, maxDate } = useMemo(() => {
    if (!commits || commits.length === 0) {
      const now = Date.now()
      return { minDate: now, maxDate: now }
    }
    
    const timestamps = commits.map(c => new Date(c.timestamp).getTime())
    return {
      minDate: Math.min(...timestamps),
      maxDate: Math.max(...timestamps),
    }
  }, [commits])
  
  // Initialize current date
  useEffect(() => {
    if (commits && commits.length > 0 && maxDate > 0) {
      setCurrentDate(maxDate)
    }
  }, [commits, maxDate])

  // Keyboard shortcuts
  useKeyboardShortcuts({
    'toggle-commits': () => setViewMode('commits'),
    'toggle-heatmap': () => setViewMode('heatmap'),
    'toggle-branches': () => setViewMode('branches'),
    'toggle-all': () => setViewMode('all'),
    'toggle-stats': () => setShowStats((prev) => !prev),
    'toggle-realtime': () => setShowRealtime((prev) => !prev),
    'toggle-help': () => setShowHelp((prev) => !prev),
    'reset-camera': () => {
      if (controlsRef.current) {
        controlsRef.current.reset()
      }
    },
  })
  
  const showToast = (message: string, type: ToastType) => {
    const id = Math.random().toString()
    setToasts(prev => [...prev, { id, message, type }])
  }
  
  const removeToast = (id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id))
  }

  return (
    <div className="app">
      <ControlPanel
        viewMode={viewMode}
        onViewModeChange={setViewMode}
        repoPath={repoPath}
        onRepoPathChange={setRepoPath}
        showStats={showStats}
        onToggleStats={() => setShowStats(!showStats)}
        onShowHelp={() => setShowHelp(true)}
      />

      {showStats && <PerformanceMonitor />}
      
      <KeyboardShortcutsHelp 
        isOpen={showHelp} 
        onClose={() => setShowHelp(false)} 
      />
      
      <SearchBar
        onFiltersChange={setSearchFilters}
        availableAuthors={availableAuthors}
        availableBranches={availableBranches}
      />
      
      {commits && commits.length > 0 && (
        <TimelineSlider
          minDate={minDate}
          maxDate={maxDate}
          currentDate={currentDate}
          onDateChange={setCurrentDate}
          commits={commits}
        />
      )}
      
      <BookmarksPanel
        isOpen={showBookmarks}
        onClose={() => setShowBookmarks(false)}
      />
      
      <ToastContainer toasts={toasts} onRemove={removeToast} />

      <Canvas
        gl={{
          antialias: true,
          alpha: false,
          powerPreference: 'high-performance',
        }}
        dpr={[1, 2]}
      >
        <PerspectiveCamera makeDefault position={[50, 50, 50]} fov={60} />
        <OrbitControls
          ref={controlsRef}
          enableDamping
          dampingFactor={0.05}
          rotateSpeed={0.5}
          zoomSpeed={0.8}
          makeDefault
        />
        
        {/* Lighting */}
        <ambientLight intensity={0.5} />
        <directionalLight position={[10, 10, 5]} intensity={1} />
        <pointLight position={[-10, -10, -10]} intensity={0.5} />

        {/* Grid helper */}
        <gridHelper args={[200, 20, '#444444', '#222222']} />

        <Suspense fallback={<LoadingScreen />}>
          {viewMode === 'commits' && <CommitGraph3D repoPath={repoPath} />}
          {viewMode === 'heatmap' && <FileHeatMap repoPath={repoPath} />}
          {viewMode === 'branches' && <BranchStructure3D repoPath={repoPath} />}
          {viewMode === 'all' && (
            <>
              <CommitGraph3D repoPath={repoPath} />
              <FileHeatMap repoPath={repoPath} />
              <BranchStructure3D repoPath={repoPath} />
            </>
          )}
        </Suspense>

      </Canvas>

      {showRealtime && <RealtimeMonitor repoPath={repoPath} />}
    </div>
  )
}

export default App

