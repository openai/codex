import { useQuery } from '@tanstack/react-query'
import { fetchCommits, fetchFileHeatmap, fetchBranchGraph } from '../lib/api'

export function useCommits(repoPath?: string, limit = 1000) {
  return useQuery({
    queryKey: ['commits', repoPath, limit],
    queryFn: () => fetchCommits(repoPath, limit),
    staleTime: 30000,
  })
}

export function useFileHeatmap(repoPath?: string, limit = 1000) {
  return useQuery({
    queryKey: ['heatmap', repoPath, limit],
    queryFn: () => fetchFileHeatmap(repoPath, limit),
    staleTime: 30000,
  })
}

export function useBranchGraph(repoPath?: string) {
  return useQuery({
    queryKey: ['branches', repoPath],
    queryFn: () => fetchBranchGraph(repoPath),
    staleTime: 30000,
  })
}

