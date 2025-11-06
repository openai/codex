/**
 * Plan Execution Hook
 * 
 * Manages Plan execution with real-time progress updates via SSE
 */

import { useState, useEffect, useCallback, useRef } from 'react'

export interface ExecutionProgress {
  type: 'started' | 'progress' | 'step_completed' | 'file_changed' | 'completed' | 'failed'
  data: any
  timestamp: string
}

export interface ExecutionState {
  isExecuting: boolean
  currentStep: number
  totalSteps: number
  message: string
  filesChanged: string[]
  testsPassed: string[]
  testsFailed: string[]
  completed: boolean
  success: boolean | null
  error: string | null
}

export function usePlanExecution(PlanId: string) {
  const [state, setState] = useState<ExecutionState>({
    isExecuting: false,
    currentStep: 0,
    totalSteps: 0,
    message: '',
    filesChanged: [],
    testsPassed: [],
    testsFailed: [],
    completed: false,
    success: null,
    error: null,
  })

  const eventSourceRef = useRef<EventSource | null>(null)

  const startExecution = useCallback(async () => {
    // Reset state
    setState({
      isExecuting: true,
      currentStep: 0,
      totalSteps: 0,
      message: 'Starting execution...',
      filesChanged: [],
      testsPassed: [],
      testsFailed: [],
      completed: false,
      success: null,
      error: null,
    })

    // Connect to SSE endpoint
    const eventSource = new EventSource(
      `/api/Plan/execute?PlanId=${PlanId}`
    )

    eventSourceRef.current = eventSource

    eventSource.onmessage = (event) => {
      try {
        const progress: ExecutionProgress = JSON.parse(event.data)

        setState((prev) => {
          const newState = { ...prev }

          switch (progress.type) {
            case 'started':
              newState.isExecuting = true
              newState.message = 'Execution started'
              break

            case 'progress':
              newState.currentStep = progress.data.current_step
              newState.totalSteps = progress.data.total_steps
              newState.message = progress.data.message
              break

            case 'step_completed':
              newState.message = `Step completed: ${progress.data.step_name}`
              break

            case 'file_changed':
              newState.filesChanged = [...prev.filesChanged, progress.data.file_path]
              newState.message = `File ${progress.data.change_type}: ${progress.data.file_path}`
              break

            case 'completed':
              newState.isExecuting = false
              newState.completed = true
              newState.success = progress.data.success
              newState.message = progress.data.message
              break

            case 'failed':
              newState.isExecuting = false
              newState.completed = true
              newState.success = false
              newState.error = progress.data.error
              newState.message = `Execution failed: ${progress.data.error}`
              break
          }

          return newState
        })
      } catch (error) {
        console.error('Failed to parse SSE message:', error)
      }
    }

    eventSource.onerror = (error) => {
      console.error('SSE error:', error)
      eventSource.close()
      setState((prev) => ({
        ...prev,
        isExecuting: false,
        completed: true,
        success: false,
        error: 'Connection error',
      }))
    }
  }, [PlanId])

  const stopExecution = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close()
      eventSourceRef.current = null
    }

    setState((prev) => ({
      ...prev,
      isExecuting: false,
    }))
  }, [])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (eventSourceRef.current) {
        eventSourceRef.current.close()
      }
    }
  }, [])

  return {
    state,
    startExecution,
    stopExecution,
  }
}
