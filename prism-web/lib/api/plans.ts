/**
 * Plan API Client
 * 
 * Communicates with Rust CLI via child process execution
 */

import { exec } from 'child_process'
import { promisify } from 'util'

const execAsync = promisify(exec)

export interface Plan {
  id: string
  title: string
  goal: string
  approach: string
  mode: 'single' | 'orchestrated' | 'competition'
  state: 'Drafting' | 'Pending' | 'Approved' | 'Rejected' | 'Executing' | 'Completed' | 'Failed'
  created_at: string
  updated_at: string
  approved_by?: string | null
  rejected_reason?: string | null
  budget: {
    session_cap?: number
    cap_min?: number
  }
  work_items: Array<{
    name: string
    files_touched: string[]
    diff_contract: string
    tests: string[]
  }>
  risks: Array<{
    item: string
    mitigation: string
  }>
}

export interface CreatePlanRequest {
  title: string
  mode?: 'single' | 'orchestrated' | 'competition'
  budget_tokens?: number
  budget_time?: number
}

/**
 * List all Plans
 */
export async function listPlans(state?: string): Promise<Plan[]> {
  try {
    const stateArg = state ? `--state ${state}` : ''
    const { stdout } = await execAsync(`codex Plan list ${stateArg} --json`)
    return JSON.parse(stdout)
  } catch (error) {
    console.error('Failed to list Plans:', error)
    return []
  }
}

/**
 * Create a new Plan
 */
export async function createPlan(data: CreatePlanRequest): Promise<Plan> {
  const {
    title,
    mode = 'orchestrated',
    budget_tokens = 100000,
    budget_time = 30,
  } = data

  const { stdout } = await execAsync(
    `codex Plan create "${title}" --mode=${mode} --budget-tokens=${budget_tokens} --budget-time=${budget_time} --json`
  )

  return JSON.parse(stdout)
}

/**
 * Get Plan status
 */
export async function getPlan(id: string): Promise<Plan> {
  const { stdout } = await execAsync(`codex Plan status ${id} --json`)
  return JSON.parse(stdout)
}

/**
 * Approve a Plan
 */
export async function approvePlan(id: string): Promise<Plan> {
  const { stdout } = await execAsync(`codex Plan approve ${id} --json`)
  return JSON.parse(stdout)
}

/**
 * Reject a Plan
 */
export async function rejectPlan(id: string, reason: string): Promise<Plan> {
  const { stdout } = await execAsync(`codex Plan reject ${id} --reason="${reason}" --json`)
  return JSON.parse(stdout)
}

/**
 * Export a Plan
 */
export async function exportPlan(
  id: string,
  format: 'md' | 'json' | 'both' = 'both'
): Promise<{ markdown?: string; json?: string }> {
  const { stdout } = await execAsync(`codex Plan export ${id} --format=${format} --json`)
  return JSON.parse(stdout)
}

/**
 * Toggle plan mode
 */
export async function togglePlanMode(enabled: boolean): Promise<void> {
  const flag = enabled ? 'on' : 'off'
  await execAsync(`codex Plan toggle ${flag}`)
}

/**
 * Get plan mode status
 */
export async function getPlanModeStatus(): Promise<{ enabled: boolean; timestamp: string }> {
  try {
    const { stdout } = await execAsync(`codex plan mode-status --json`)
    return JSON.parse(stdout)
  } catch (error) {
    return { enabled: false, timestamp: new Date().toISOString() }
  }
}

