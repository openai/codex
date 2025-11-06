'use client'

import { useEffect, useState } from 'react'
import {
  listPlans,
  createPlan,
  approvePlan,
  rejectPlan,
  exportPlan,
  togglePlanMode,
  type Plan,
  type CreatePlanRequest,
} from '../../../lib/api/Plans'
import { useplanStore } from '../../../lib/stores/planStore'

export default function PlansPage() {
  const {
    isEnabled,
    Plans,
    selectedPlan,
    loading,
    error,
    setEnabled,
    setPlans,
    setSelectedPlan,
    setLoading,
    setError,
  } = useplanStore()

  const [showCreateModal, setShowCreateModal] = useState(false)
  const [filterState, setFilterState] = useState<string | undefined>(undefined)

  useEffect(() => {
    loadPlans()
  }, [filterState])

  const loadPlans = async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await listPlans(filterState)
      setPlans(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load Plans')
    } finally {
      setLoading(false)
    }
  }

  const handleToggleMode = async () => {
    try {
      const newState = !isEnabled
      await togglePlanMode(newState)
      setEnabled(newState)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to toggle mode')
    }
  }

  const handleCreatePlan = async (data: CreatePlanRequest) => {
    try {
      await createPlan(data)
      await loadPlans()
      setShowCreateModal(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create Plan')
    }
  }

  const handleApprove = async (id: string) => {
    try {
      await approvePlan(id)
      await loadPlans()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to approve Plan')
    }
  }

  const handleReject = async (id: string, reason: string) => {
    try {
      await rejectPlan(id, reason)
      await loadPlans()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reject Plan')
    }
  }

  const handleExport = async (id: string, format: 'md' | 'json' | 'both') => {
    try {
      const result = await exportPlan(id, format)
      console.log('Export result:', result)
      // Download logic here
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to export Plan')
    }
  }

  const getStatusIcon = (state: Plan['state']) => {
    switch (state) {
      case 'Drafting':
        return '📝'
      case 'Pending':
        return '⏳'
      case 'Approved':
        return '✅'
      case 'Rejected':
        return '❌'
      case 'Executing':
        return '🚀'
      case 'Completed':
        return '🎉'
      case 'Failed':
        return '💥'
      default:
        return '❓'
    }
  }

  const getStatusColor = (state: Plan['state']) => {
    switch (state) {
      case 'Drafting':
        return 'text-yellow-400'
      case 'Pending':
        return 'text-orange-400'
      case 'Approved':
        return 'text-green-400'
      case 'Rejected':
        return 'text-red-400'
      case 'Executing':
        return 'text-blue-400'
      case 'Completed':
        return 'text-purple-400'
      case 'Failed':
        return 'text-red-600'
      default:
        return 'text-gray-400'
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-4xl font-bold text-white mb-2">plan mode</h1>
            <p className="text-gray-400">Plan, approve, and execute changes safely</p>
          </div>

          <div className="flex items-center gap-4">
            {/* Mode Toggle */}
            <button
              onClick={handleToggleMode}
              className={`px-6 py-3 rounded-lg font-semibold transition ${
                isEnabled
                  ? 'bg-green-500 hover:bg-green-600 text-white'
                  : 'bg-gray-700 hover:bg-gray-600 text-gray-300'
              }`}
            >
              {isEnabled ? '✅ Mode: ON' : '⭕ Mode: OFF'}
            </button>

            {/* Create Plan */}
            <button
              onClick={() => setShowCreateModal(true)}
              className="px-6 py-3 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-semibold transition"
            >
              ➕ Create Plan
            </button>
          </div>
        </div>

        {/* Error Display */}
        {error && (
          <div className="bg-red-500/20 border border-red-500 text-red-200 px-4 py-3 rounded-lg mb-6">
            ❌ {error}
          </div>
        )}

        {/* Filter Tabs */}
        <div className="flex gap-2 mb-6">
          {['All', 'Drafting', 'Pending', 'Approved', 'Rejected'].map((state) => (
            <button
              key={state}
              onClick={() => setFilterState(state === 'All' ? undefined : state)}
              className={`px-4 py-2 rounded-lg transition ${
                (state === 'All' && !filterState) || filterState === state
                  ? 'bg-purple-500 text-white'
                  : 'bg-gray-800 text-gray-400 hover:bg-gray-700'
              }`}
            >
              {state}
            </button>
          ))}
        </div>

        {/* Plans Grid */}
        {loading ? (
          <div className="text-center text-gray-400 py-20">
            <div className="animate-spin text-6xl mb-4">⏳</div>
            <p>Loading Plans...</p>
          </div>
        ) : Plans.length === 0 ? (
          <div className="text-center text-gray-400 py-20">
            <div className="text-6xl mb-4">📋</div>
            <p className="text-xl">No Plans found</p>
            <p className="text-sm mt-2">Create your first Plan to get started</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {Plans.map((Plan) => (
              <div
                key={Plan.id}
                className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6 hover:border-purple-500 transition cursor-pointer"
                onClick={() => setSelectedPlan(Plan)}
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-center gap-2">
                    <span className="text-3xl">{getStatusIcon(Plan.state)}</span>
                    <span className={`text-sm font-semibold ${getStatusColor(Plan.state)}`}>
                      {Plan.state}
                    </span>
                  </div>
                  <span className="text-xs text-gray-500 bg-gray-700 px-2 py-1 rounded">
                    {Plan.mode}
                  </span>
                </div>

                <h3 className="text-xl font-bold text-white mb-2 truncate">{Plan.title}</h3>
                <p className="text-sm text-gray-400 mb-4 line-clamp-2">{Plan.goal}</p>

                <div className="flex items-center justify-between text-xs text-gray-500">
                  <span>
                    💰 {Plan.budget.session_cap?.toLocaleString() || 'N/A'} tokens
                  </span>
                  <span>⏱️ {Plan.budget.cap_min || 'N/A'} min</span>
                </div>

                <div className="mt-4 pt-4 border-t border-gray-700">
                  <div className="flex gap-2">
                    {Plan.state === 'Pending' && (
                      <>
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            handleApprove(Plan.id)
                          }}
                          className="flex-1 px-3 py-2 bg-green-500 hover:bg-green-600 text-white rounded text-sm font-semibold transition"
                        >
                          ✅ Approve
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            const reason = prompt('Rejection reason:')
                            if (reason) handleReject(Plan.id, reason)
                          }}
                          className="flex-1 px-3 py-2 bg-red-500 hover:bg-red-600 text-white rounded text-sm font-semibold transition"
                        >
                          ❌ Reject
                        </button>
                      </>
                    )}

                    <button
                      onClick={(e) => {
                        e.stopPropagation()
                        handleExport(Plan.id, 'both')
                      }}
                      className="flex-1 px-3 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded text-sm transition"
                    >
                      📥 Export
                    </button>
                  </div>
                </div>

                <div className="mt-2 text-xs text-gray-600">
                  Created: {new Date(Plan.created_at).toLocaleString()}
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Create Modal */}
        {showCreateModal && (
          <CreatePlanModal
            onClose={() => setShowCreateModal(false)}
            onCreate={handleCreatePlan}
          />
        )}

        {/* Detail Modal */}
        {selectedPlan && (
          <PlanDetailModal
            Plan={selectedPlan}
            onClose={() => setSelectedPlan(null)}
            onApprove={handleApprove}
            onReject={handleReject}
            onExport={handleExport}
          />
        )}
      </div>
    </div>
  )
}

// Create Plan Modal
function CreatePlanModal({
  onClose,
  onCreate,
}: {
  onClose: () => void
  onCreate: (data: CreatePlanRequest) => void
}) {
  const [formData, setFormData] = useState<CreatePlanRequest>({
    title: '',
    mode: 'orchestrated',
    budget_tokens: 100000,
    budget_time: 30,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onCreate(formData)
  }

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-gray-800 rounded-xl p-8 max-w-2xl w-full">
        <h2 className="text-2xl font-bold text-white mb-6">Create New Plan</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-semibold text-gray-300 mb-2">Title</label>
            <input
              type="text"
              value={formData.title}
              onChange={(e) => setFormData({ ...formData, title: e.target.value })}
              className="w-full px-4 py-2 bg-gray-700 text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500"
              placeholder="e.g., Add JWT authentication"
              required
            />
          </div>

          <div>
            <label className="block text-sm font-semibold text-gray-300 mb-2">Execution Mode</label>
            <select
              value={formData.mode}
              onChange={(e) =>
                setFormData({ ...formData, mode: e.target.value as any })
              }
              className="w-full px-4 py-2 bg-gray-700 text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500"
            >
              <option value="single">Single (Simple task, no orchestration)</option>
              <option value="orchestrated">Orchestrated (Multi-agent, recommended)</option>
              <option value="competition">Competition (Performance optimization)</option>
            </select>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-semibold text-gray-300 mb-2">
                Token Budget
              </label>
              <input
                type="number"
                value={formData.budget_tokens}
                onChange={(e) =>
                  setFormData({ ...formData, budget_tokens: parseInt(e.target.value) })
                }
                className="w-full px-4 py-2 bg-gray-700 text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500"
                min="1000"
                step="1000"
              />
            </div>

            <div>
              <label className="block text-sm font-semibold text-gray-300 mb-2">
                Time Budget (minutes)
              </label>
              <input
                type="number"
                value={formData.budget_time}
                onChange={(e) =>
                  setFormData({ ...formData, budget_time: parseInt(e.target.value) })
                }
                className="w-full px-4 py-2 bg-gray-700 text-white rounded-lg focus:outline-none focus:ring-2 focus:ring-purple-500"
                min="1"
              />
            </div>
          </div>

          <div className="flex gap-4 mt-6">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 px-6 py-3 bg-gray-700 hover:bg-gray-600 text-white rounded-lg font-semibold transition"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="flex-1 px-6 py-3 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-semibold transition"
            >
              Create Plan
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

// Plan Detail Modal
function PlanDetailModal({
  Plan,
  onClose,
  onApprove,
  onReject,
  onExport,
}: {
  Plan: Plan
  onClose: () => void
  onApprove: (id: string) => void
  onReject: (id: string, reason: string) => void
  onExport: (id: string, format: 'md' | 'json' | 'both') => void
}) {
  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center p-4 z-50">
      <div className="bg-gray-800 rounded-xl p-8 max-w-4xl w-full max-h-[90vh] overflow-y-auto">
        <div className="flex items-start justify-between mb-6">
          <div>
            <h2 className="text-3xl font-bold text-white mb-2">{Plan.title}</h2>
            <div className="flex items-center gap-4 text-sm">
              <span className={`font-semibold ${getStatusColor(Plan.state)}`}>
                {getStatusIcon(Plan.state)} {Plan.state}
              </span>
              <span className="text-gray-400">Mode: {Plan.mode}</span>
              <span className="text-gray-400">ID: {Plan.id}</span>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white text-2xl"
          >
            ×
          </button>
        </div>

        <div className="space-y-6">
          <div>
            <h3 className="text-xl font-semibold text-white mb-2">Goal</h3>
            <p className="text-gray-300">{Plan.goal}</p>
          </div>

          <div>
            <h3 className="text-xl font-semibold text-white mb-2">Approach</h3>
            <p className="text-gray-300">{Plan.approach}</p>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="bg-gray-700/50 p-4 rounded-lg">
              <div className="text-sm text-gray-400 mb-1">Token Budget</div>
              <div className="text-2xl font-bold text-white">
                {Plan.budget.session_cap?.toLocaleString() || 'N/A'}
              </div>
            </div>
            <div className="bg-gray-700/50 p-4 rounded-lg">
              <div className="text-sm text-gray-400 mb-1">Time Budget</div>
              <div className="text-2xl font-bold text-white">
                {Plan.budget.cap_min || 'N/A'} min
              </div>
            </div>
          </div>

          <div>
            <h3 className="text-xl font-semibold text-white mb-2">Work Items</h3>
            {Plan.work_items.length === 0 ? (
              <p className="text-gray-400 italic">None specified</p>
            ) : (
              <ul className="space-y-2">
                {Plan.work_items.map((item, i) => (
                  <li key={i} className="bg-gray-700/50 p-3 rounded">
                    <div className="font-semibold text-white">{item.name}</div>
                    <div className="text-sm text-gray-400">
                      Files: {item.files_touched.join(', ')}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div>
            <h3 className="text-xl font-semibold text-white mb-2">Risks</h3>
            {Plan.risks.length === 0 ? (
              <p className="text-gray-400 italic">None identified</p>
            ) : (
              <ul className="space-y-2">
                {Plan.risks.map((risk, i) => (
                  <li key={i} className="bg-gray-700/50 p-3 rounded">
                    <div className="font-semibold text-red-400">⚠️ {risk.item}</div>
                    <div className="text-sm text-gray-300">
                      Mitigation: {risk.mitigation}
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="flex gap-4 mt-6">
            {Plan.state === 'Pending' && (
              <>
                <button
                  onClick={() => onApprove(Plan.id)}
                  className="flex-1 px-6 py-3 bg-green-500 hover:bg-green-600 text-white rounded-lg font-semibold transition"
                >
                  ✅ Approve
                </button>
                <button
                  onClick={() => {
                    const reason = prompt('Rejection reason:')
                    if (reason) onReject(Plan.id, reason)
                  }}
                  className="flex-1 px-6 py-3 bg-red-500 hover:bg-red-600 text-white rounded-lg font-semibold transition"
                >
                  ❌ Reject
                </button>
              </>
            )}

            <button
              onClick={() => onExport(Plan.id, 'both')}
              className="px-6 py-3 bg-gray-700 hover:bg-gray-600 text-white rounded-lg font-semibold transition"
            >
              📥 Export
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function getStatusIcon(state: Plan['state']) {
  switch (state) {
    case 'Drafting':
      return '📝'
    case 'Pending':
      return '⏳'
    case 'Approved':
      return '✅'
    case 'Rejected':
      return '❌'
    case 'Executing':
      return '🚀'
    case 'Completed':
      return '🎉'
    case 'Failed':
      return '💥'
    default:
      return '❓'
  }
}

function getStatusColor(state: Plan['state']) {
  switch (state) {
    case 'Drafting':
      return 'text-yellow-400'
    case 'Pending':
      return 'text-orange-400'
    case 'Approved':
      return 'text-green-400'
    case 'Rejected':
      return 'text-red-400'
    case 'Executing':
      return 'text-blue-400'
    case 'Completed':
      return 'text-purple-400'
    case 'Failed':
      return 'text-red-600'
    default:
      return 'text-gray-400'
  }
}

