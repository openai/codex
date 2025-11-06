'use client'

import { useState, useEffect } from 'react'
import { useParams, useRouter } from 'next/navigation'
import { getPlan, type Plan } from '../../../../../lib/api/Plans'
import { usePlanExecution } from '../../../../../lib/hooks/usePlanExecution'

export default function PlanExecutePage() {
  const params = useParams()
  const router = useRouter()
  const PlanId = params.id as string

  const [Plan, setPlan] = useState<Plan | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const { state: execState, startExecution, stopExecution } = usePlanExecution(PlanId)

  useEffect(() => {
    loadPlan()
  }, [PlanId])

  const loadPlan = async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await getPlan(PlanId)
      setPlan(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load Plan')
    } finally {
      setLoading(false)
    }
  }

  const handleStartExecution = async () => {
    await startExecution()
  }

  const handleCancelExecution = () => {
    stopExecution()
  }

  if (loading) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8 flex items-center justify-center">
        <div className="text-center text-gray-400">
          <div className="animate-spin text-6xl mb-4">⏳</div>
          <p>Loading Plan...</p>
        </div>
      </div>
    )
  }

  if (error || !Plan) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8 flex items-center justify-center">
        <div className="text-center">
          <div className="text-6xl mb-4">❌</div>
          <p className="text-red-400 text-xl">{error || 'Plan not found'}</p>
          <button
            onClick={() => router.push('/Plans')}
            className="mt-6 px-6 py-3 bg-gray-700 hover:bg-gray-600 text-white rounded-lg"
          >
            ← Back to Plans
          </button>
        </div>
      </div>
    )
  }

  const progressPercentage =
    execState.totalSteps > 0 ? (execState.currentStep / execState.totalSteps) * 100 : 0

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <button
              onClick={() => router.push('/Plans')}
              className="text-gray-400 hover:text-white mb-4 flex items-center gap-2"
            >
              ← Back to Plans
            </button>
            <h1 className="text-4xl font-bold text-white mb-2">{Plan.title}</h1>
            <p className="text-gray-400">Plan Execution</p>
          </div>

          <div className="flex items-center gap-4">
            {!execState.isExecuting && !execState.completed && (
              <button
                onClick={handleStartExecution}
                disabled={Plan.state !== 'Approved'}
                className="px-8 py-4 bg-green-500 hover:bg-green-600 disabled:bg-gray-700 disabled:text-gray-500 text-white rounded-lg font-bold text-lg transition shadow-lg"
              >
                🚀 Start Execution
              </button>
            )}

            {execState.isExecuting && (
              <button
                onClick={handleCancelExecution}
                className="px-8 py-4 bg-red-500 hover:bg-red-600 text-white rounded-lg font-bold text-lg transition shadow-lg"
              >
                🛑 Cancel
              </button>
            )}

            {execState.completed && (
              <button
                onClick={() => router.push('/Plans')}
                className="px-8 py-4 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-bold text-lg transition shadow-lg"
              >
                ✅ Done
              </button>
            )}
          </div>
        </div>

        {/* Execution Status Card */}
        <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-2xl font-bold text-white">Execution Status</h2>
            <div className="flex items-center gap-3">
              {execState.isExecuting && (
                <span className="flex items-center gap-2 text-blue-400">
                  <span className="animate-spin">🚀</span>
                  <span className="font-semibold">Executing...</span>
                </span>
              )}
              {execState.completed && execState.success && (
                <span className="flex items-center gap-2 text-green-400">
                  <span>🎉</span>
                  <span className="font-semibold">Completed</span>
                </span>
              )}
              {execState.completed && !execState.success && (
                <span className="flex items-center gap-2 text-red-400">
                  <span>💥</span>
                  <span className="font-semibold">Failed</span>
                </span>
              )}
            </div>
          </div>

          {/* Progress Bar */}
          {execState.totalSteps > 0 && (
            <div className="mb-4">
              <div className="flex items-center justify-between text-sm text-gray-400 mb-2">
                <span>
                  Step {execState.currentStep} of {execState.totalSteps}
                </span>
                <span>{progressPercentage.toFixed(0)}%</span>
              </div>
              <div className="w-full bg-gray-700 rounded-full h-3 overflow-hidden">
                <div
                  className="bg-gradient-to-r from-purple-500 to-blue-500 h-full transition-all duration-300"
                  style={{ width: `${progressPercentage}%` }}
                />
              </div>
            </div>
          )}

          {/* Current Message */}
          {execState.message && (
            <div className="bg-gray-700/50 p-4 rounded-lg">
              <div className="text-sm text-gray-400 mb-1">Current Status</div>
              <div className="text-white">{execState.message}</div>
            </div>
          )}

          {/* Error Display */}
          {execState.error && (
            <div className="bg-red-500/20 border border-red-500 text-red-200 p-4 rounded-lg mt-4">
              <div className="font-semibold mb-1">Error</div>
              <div className="text-sm">{execState.error}</div>
            </div>
          )}
        </div>

        {/* Files Changed */}
        {execState.filesChanged.length > 0 && (
          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6 mb-6">
            <h2 className="text-2xl font-bold text-white mb-4">Files Changed</h2>
            <div className="space-y-2">
              {execState.filesChanged.map((file, i) => (
                <div
                  key={i}
                  className="bg-gray-700/50 p-3 rounded flex items-center gap-3"
                >
                  <span className="text-green-400">✓</span>
                  <span className="text-white font-mono text-sm">{file}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Tests Results */}
        {(execState.testsPassed.length > 0 || execState.testsFailed.length > 0) && (
          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6 mb-6">
            <h2 className="text-2xl font-bold text-white mb-4">Test Results</h2>
            
            <div className="grid grid-cols-2 gap-4 mb-4">
              <div className="bg-green-500/20 border border-green-500/50 rounded-lg p-4">
                <div className="text-3xl font-bold text-green-400">
                  {execState.testsPassed.length}
                </div>
                <div className="text-sm text-green-300">Tests Passed</div>
              </div>
              <div className="bg-red-500/20 border border-red-500/50 rounded-lg p-4">
                <div className="text-3xl font-bold text-red-400">
                  {execState.testsFailed.length}
                </div>
                <div className="text-sm text-red-300">Tests Failed</div>
              </div>
            </div>

            {execState.testsFailed.length > 0 && (
              <div className="space-y-2">
                <h3 className="text-lg font-semibold text-red-400">Failed Tests</h3>
                {execState.testsFailed.map((test, i) => (
                  <div key={i} className="bg-red-500/10 border border-red-500/50 p-3 rounded">
                    <span className="text-red-300 font-mono text-sm">{test}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Plan Info */}
        <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
          <h2 className="text-2xl font-bold text-white mb-4">Plan Info</h2>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <div className="text-sm text-gray-400 mb-1">ID</div>
              <div className="text-white font-mono text-sm">{Plan.id}</div>
            </div>
            <div>
              <div className="text-sm text-gray-400 mb-1">Mode</div>
              <div className="text-white">{Plan.mode}</div>
            </div>
            <div>
              <div className="text-sm text-gray-400 mb-1">Token Budget</div>
              <div className="text-white">
                {Plan.budget.session_cap?.toLocaleString() || 'N/A'}
              </div>
            </div>
            <div>
              <div className="text-sm text-gray-400 mb-1">Time Budget</div>
              <div className="text-white">{Plan.budget.cap_min || 'N/A'} min</div>
            </div>
          </div>

          <div className="mt-4">
            <div className="text-sm text-gray-400 mb-1">Goal</div>
            <div className="text-white">{Plan.goal}</div>
          </div>

          {Plan.work_items.length > 0 && (
            <div className="mt-4">
              <div className="text-sm text-gray-400 mb-2">Work Items</div>
              <div className="space-y-2">
                {Plan.work_items.map((item, i) => (
                  <div key={i} className="bg-gray-700/50 p-3 rounded">
                    <div className="text-white font-semibold">{item.name}</div>
                    <div className="text-sm text-gray-400">
                      Files: {item.files_touched.join(', ')}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
