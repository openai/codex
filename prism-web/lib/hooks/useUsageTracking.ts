/**
 * Usage Tracking Hook
 * 
 * Tracks AI API usage and costs
 */

import { useState, useEffect, useCallback } from 'react'
import { createClientComponentClient } from '@supabase/auth-helpers-nextjs'

export interface UsageRecord {
  id: string
  user_id: string
  model: string
  tokens: number
  cost: number
  created_at: string
}

export interface UsageSummary {
  total_tokens: number
  total_cost: number
  by_model: Record<string, { tokens: number; cost: number; count: number }>
  this_month: number
  last_month: number
}

export function useUsageTracking() {
  const [usage, setUsage] = useState<UsageRecord[]>([])
  const [summary, setSummary] = useState<UsageSummary | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const supabase = createClientComponentClient()

  const trackUsage = useCallback(
    async (model: string, tokens: number, cost: number) => {
      try {
        const {
          data: { user },
        } = await supabase.auth.getUser()

        if (!user) throw new Error('Not authenticated')

        const { error } = await supabase.from('usage_logs').insert({
          user_id: user.id,
          model,
          tokens,
          cost,
        })

        if (error) throw error
      } catch (err) {
        console.error('Failed to track usage:', err)
        setError(err instanceof Error ? err.message : 'Failed to track usage')
      }
    },
    [supabase]
  )

  const loadUsage = useCallback(
    async (startDate?: Date, endDate?: Date) => {
      setLoading(true)
      setError(null)

      try {
        const {
          data: { user },
        } = await supabase.auth.getUser()

        if (!user) throw new Error('Not authenticated')

        let query = supabase
          .from('usage_logs')
          .select('*')
          .eq('user_id', user.id)
          .order('created_at', { ascending: false })

        if (startDate) {
          query = query.gte('created_at', startDate.toISOString())
        }

        if (endDate) {
          query = query.lte('created_at', endDate.toISOString())
        }

        const { data, error } = await query

        if (error) throw error

        setUsage(data || [])

        // Calculate summary
        const byModel: Record<string, { tokens: number; cost: number; count: number }> = {}
        let totalTokens = 0
        let totalCost = 0

        data?.forEach((record) => {
          totalTokens += record.tokens
          totalCost += record.cost

          if (!byModel[record.model]) {
            byModel[record.model] = { tokens: 0, cost: 0, count: 0 }
          }

          byModel[record.model].tokens += record.tokens
          byModel[record.model].cost += record.cost
          byModel[record.model].count += 1
        })

        // Calculate this month and last month
        const now = new Date()
        const thisMonthStart = new Date(now.getFullYear(), now.getMonth(), 1)
        const lastMonthStart = new Date(now.getFullYear(), now.getMonth() - 1, 1)
        const lastMonthEnd = new Date(now.getFullYear(), now.getMonth(), 0)

        const thisMonthRecords = data?.filter(
          (r) => new Date(r.created_at) >= thisMonthStart
        )
        const lastMonthRecords = data?.filter(
          (r) =>
            new Date(r.created_at) >= lastMonthStart &&
            new Date(r.created_at) <= lastMonthEnd
        )

        setSummary({
          total_tokens: totalTokens,
          total_cost: totalCost,
          by_model: byModel,
          this_month: thisMonthRecords?.reduce((sum, r) => sum + r.cost, 0) || 0,
          last_month: lastMonthRecords?.reduce((sum, r) => sum + r.cost, 0) || 0,
        })
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load usage')
      } finally {
        setLoading(false)
      }
    },
    [supabase]
  )

  useEffect(() => {
    loadUsage()
  }, [loadUsage])

  return {
    usage,
    summary,
    loading,
    error,
    trackUsage,
    loadUsage,
  }
}
