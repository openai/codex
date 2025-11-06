/**
 * Plan Store (Zustand)
 * 
 * Global state management for plan mode
 */

import { create } from 'zustand'
import type { Plan } from '../api/Plans'

interface planStore {
  // State
  isEnabled: boolean
  Plans: Plan[]
  selectedPlan: Plan | null
  loading: boolean
  error: string | null

  // Actions
  setEnabled: (enabled: boolean) => void
  setPlans: (Plans: Plan[]) => void
  setSelectedPlan: (Plan: Plan | null) => void
  setLoading: (loading: boolean) => void
  setError: (error: string | null) => void
  
  // Computed
  getDraftingPlans: () => Plan[]
  getPendingPlans: () => Plan[]
  getApprovedPlans: () => Plan[]
  getRejectedPlans: () => Plan[]
}

export const useplanStore = create<planStore>((set, get) => ({
  // Initial state
  isEnabled: false,
  Plans: [],
  selectedPlan: null,
  loading: false,
  error: null,

  // Actions
  setEnabled: (enabled) => set({ isEnabled: enabled }),
  
  setPlans: (Plans) => set({ Plans }),
  
  setSelectedPlan: (Plan) => set({ selectedPlan: Plan }),
  
  setLoading: (loading) => set({ loading }),
  
  setError: (error) => set({ error }),

  // Computed getters
  getDraftingPlans: () => 
    get().Plans.filter((bp) => bp.state === 'Drafting'),
  
  getPendingPlans: () =>
    get().Plans.filter((bp) => bp.state === 'Pending'),
  
  getApprovedPlans: () =>
    get().Plans.filter((bp) => bp.state === 'Approved'),
  
  getRejectedPlans: () =>
    get().Plans.filter((bp) => bp.state === 'Rejected'),
}))

