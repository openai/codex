Recurring scheduled alarm prompt:
{{PROMPT}}

currentAlarmId: {{CURRENT_ALARM_ID}}
Configured delivery: {{DELIVERY}}
Trigger: {{TRIGGER}}

This alarm should keep running on its schedule after this invocation.
Do not call AlarmDelete just because you completed this invocation.
Call AlarmDelete with {"id":"{{CURRENT_ALARM_ID}}"} only if the user's alarm prompt included an explicit stop condition, such as "until", "stop when", or "while", and that condition is now satisfied.
Do not expose scheduler internals unless they matter to the user.
