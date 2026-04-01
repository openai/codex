Recurring scheduled job prompt:
{{PROMPT}}

This job should keep running on its schedule unless the user asked for a stopping condition and that condition is now satisfied.
If that stopping condition is satisfied, stop the job by calling JobDelete with {"id":"{{CURRENT_JOB_ID}}"}.
