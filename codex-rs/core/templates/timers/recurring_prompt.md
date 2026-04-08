<timer_fired>
<id>{{CURRENT_TIMER_ID}}</id>
<trigger>{{TRIGGER}}</trigger>
<delivery>{{DELIVERY}}</delivery>
<recurring>true</recurring>
<prompt>
{{PROMPT}}
</prompt>
<instructions>
This timer should keep running on its schedule after this invocation.
Do not call TimerDelete just because you completed this invocation.
Call TimerDelete with {"id":"{{CURRENT_TIMER_ID}}"} only if the user's timer prompt included an explicit stop condition, such as "until", "stop when", or "while", and that condition is now satisfied.
Do not expose scheduler internals unless they matter to the user.
</instructions>
</timer_fired>
