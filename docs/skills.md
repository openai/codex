# Skills

For information about skills, refer to [this documentation](https://developers.openai.com/codex/skills).

Local skills can opt out of implicit prompt injection by setting
`policy.allow_implicit_invocation = false` in `agents/openai.yaml`, while still
supporting explicit `$skill` mentions.
