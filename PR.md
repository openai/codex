## Summary
- thread fallback evaluation through execpolicy so pipelines evaluate the first unmatched command instead of stopping at the first allow match
- update core exec_policy integration, execpolicycheck CLI, and docs to use the new fallback path and avoid skipping heuristics on later piped commands

## Testing
- not run (not requested)
