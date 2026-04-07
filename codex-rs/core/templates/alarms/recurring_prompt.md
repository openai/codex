Recurring scheduled alarm prompt:
{{PROMPT}}

currentAlarmId: {{CURRENT_ALARM_ID}}
Configured delivery: {{DELIVERY}}
Trigger: {{TRIGGER}}

This alarm should keep running on its schedule unless the user asked for a stopping condition and that condition is now satisfied.
If that stopping condition is satisfied, stop the alarm by calling AlarmDelete with {"id":"{{CURRENT_ALARM_ID}}"}.
Do not expose scheduler internals unless they matter to the user.
