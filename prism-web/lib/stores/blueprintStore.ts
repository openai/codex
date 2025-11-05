/**
 * Blueprint Store (Zustand)
 * 
 * Global state management for Blueprint Mode
 */

import { create } from 'zustand'
import type { Blueprint } from '../api/blueprints'

interface BlueprintStore {
  // State
  isEnabled: boolean
  blueprints: Blueprint[]
  selectedBlueprint: Blueprint | null
  loading: boolean
  error: string | null

  // Actions
  setEnabled: (enabled: boolean) => void
  setBlueprints: (blueprints: Blueprint[]) => void
  setSelectedBlueprint: (blueprint: Blueprint | null) => void
  setLoading: (loading: boolean) => void
  setError: (error: string | null) => void
  
  // Computed
  getDraftingBlueprints: () => Blueprint[]
  getPendingBlueprints: () => Blueprint[]
  getApprovedBlueprints: () => Blueprint[]
  getRejectedBlueprints: () => Blueprint[]
}

export const useBlueprintStore = create<BlueprintStore>((set, get) => ({
  // Initial state
  isEnabled: false,
  blueprints: [],
  selectedBlueprint: null,
  loading: false,
  error: null,

  // Actions
  setEnabled: (enabled) => set({ isEnabled: enabled }),
  
  setBlueprints: (blueprints) => set({ blueprints }),
  
  setSelectedBlueprint: (blueprint) => set({ selectedBlueprint: blueprint }),
  
  setLoading: (loading) => set({ loading }),
  
  setError: (error) => set({ error }),

  // Computed getters
  getDraftingBlueprints: () => 
    get().blueprints.filter((bp) => bp.state === 'Drafting'),
  
  getPendingBlueprints: () =>
    get().blueprints.filter((bp) => bp.state === 'Pending'),
  
  getApprovedBlueprints: () =>
    get().blueprints.filter((bp) => bp.state === 'Approved'),
  
  getRejectedBlueprints: () =>
    get().blueprints.filter((bp) => bp.state === 'Rejected'),
}))

