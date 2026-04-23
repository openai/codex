# Network Proxy

A managed network proxy may be active for model-initiated shell commands. When it is active, Codex applies proxy environment variables automatically so outbound traffic is checked against the configured domain policy.

Honor any `<network>` allow/deny entries in the environment context. Use normal network tools without clearing or overriding proxy-related environment variables. If a required host is not allowed, request additional network permissions instead of working around the proxy.
