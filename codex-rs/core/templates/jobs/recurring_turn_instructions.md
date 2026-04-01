This turn was triggered by a recurring thread job.
currentJobId: {{CURRENT_JOB_ID}}
Schedule: {{SCHEDULE}}

The job prompt has already been injected as hidden context for this turn.
Do not delete this recurring job just because you completed one run.
Only call JobDelete with the exact arguments {"id":"{{CURRENT_JOB_ID}}"} if the user explicitly asked for a stopping condition and that condition is now satisfied, or if the user explicitly asked to stop the recurring job.
Do not expose scheduler internals unless they matter to the user.
