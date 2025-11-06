'use client'

import { useState } from 'react'
import { useUsageTracking } from '../../../lib/hooks/useUsageTracking'
import { Line } from 'react-chartjs-2'
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js'

// Register Chart.js components
ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend)

export default function UsagePage() {
  const { usage, summary, loading, error } = useUsageTracking()
  const [dateRange, setDateRange] = useState<'7d' | '30d' | '90d' | 'all'>('30d')

  if (loading) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8 flex items-center justify-center">
        <div className="text-center text-gray-400">
          <div className="animate-spin text-6xl mb-4">竢ｳ</div>
          <p>Loading usage data...</p>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8 flex items-center justify-center">
        <div className="text-center">
          <div className="text-6xl mb-4">笶・/div>
          <p className="text-red-400 text-xl">{error}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-900 via-purple-900 to-gray-900 p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div>
            <h1 className="text-4xl font-bold text-white mb-2">Usage & Billing</h1>
            <p className="text-gray-400">Track your AI API usage and costs</p>
          </div>

          <select
            value={dateRange}
            onChange={(e) => setDateRange(e.target.value as any)}
            className="px-4 py-2 bg-gray-800 text-white rounded-lg border border-gray-700 focus:outline-none focus:ring-2 focus:ring-purple-500"
          >
            <option value="7d">Last 7 days</option>
            <option value="30d">Last 30 days</option>
            <option value="90d">Last 90 days</option>
            <option value="all">All time</option>
          </select>
        </div>

        {/* Summary Cards */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-6 mb-8">
          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
            <div className="text-sm text-gray-400 mb-1">Total Cost</div>
            <div className="text-3xl font-bold text-white">
              ${summary?.total_cost.toFixed(2) || '0.00'}
            </div>
          </div>

          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
            <div className="text-sm text-gray-400 mb-1">This Month</div>
            <div className="text-3xl font-bold text-green-400">
              ${summary?.this_month.toFixed(2) || '0.00'}
            </div>
          </div>

          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
            <div className="text-sm text-gray-400 mb-1">Last Month</div>
            <div className="text-3xl font-bold text-gray-400">
              ${summary?.last_month.toFixed(2) || '0.00'}
            </div>
          </div>

          <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
            <div className="text-sm text-gray-400 mb-1">Total Tokens</div>
            <div className="text-3xl font-bold text-purple-400">
              {summary?.total_tokens.toLocaleString() || '0'}
            </div>
          </div>
        </div>

        {/* Usage by Model */}
        <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6 mb-8">
          <h2 className="text-2xl font-bold text-white mb-4">Usage by Model</h2>
          <div className="space-y-4">
            {summary &&
              Object.entries(summary.by_model).map(([model, stats]) => (
                <div
                  key={model}
                  className="bg-gray-700/50 p-4 rounded-lg flex items-center justify-between"
                >
                  <div>
                    <div className="text-white font-semibold">{model}</div>
                    <div className="text-sm text-gray-400">
                      {stats.count} requests ﾂｷ {stats.tokens.toLocaleString()} tokens
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="text-2xl font-bold text-white">${stats.cost.toFixed(2)}</div>
                  </div>
                </div>
              ))}
          </div>
        </div>

        {/* Recent Usage */}
        <div className="bg-gray-800/50 backdrop-blur-lg border border-gray-700 rounded-xl p-6">
          <h2 className="text-2xl font-bold text-white mb-4">Recent Usage</h2>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="text-left border-b border-gray-700">
                  <th className="pb-3 text-sm font-semibold text-gray-400">Date</th>
                  <th className="pb-3 text-sm font-semibold text-gray-400">Model</th>
                  <th className="pb-3 text-sm font-semibold text-gray-400 text-right">
                    Tokens
                  </th>
                  <th className="pb-3 text-sm font-semibold text-gray-400 text-right">Cost</th>
                </tr>
              </thead>
              <tbody>
                {usage.slice(0, 20).map((record) => (
                  <tr key={record.id} className="border-b border-gray-700/50">
                    <td className="py-3 text-sm text-gray-300">
                      {new Date(record.created_at).toLocaleString()}
                    </td>
                    <td className="py-3 text-sm text-white font-mono">{record.model}</td>
                    <td className="py-3 text-sm text-gray-300 text-right">
                      {record.tokens.toLocaleString()}
                    </td>
                    <td className="py-3 text-sm text-white font-semibold text-right">
                      ${record.cost.toFixed(4)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  )
}
