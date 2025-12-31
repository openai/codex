# Subagents

Codex CLI includes native subagents that the main agent can call through tools
to offload investigation and complex workflows to isolated contexts.

## Tools

### spawn_subagents (parallel)

Runs multiple subagents in parallel and waits for all results.

Parameters:

- `tasks`: array of task objects
  - `prompt` (string, required): task to run
  - `type` (string, optional): `explore`, `plan`, or `general` (default `explore`)
  - `thoroughness` (string, optional): `quick`, `medium`, or `thorough` (default `medium`)
  - `resume` (string, optional): resume a prior subagent by agentId

Returns: array of results, each with `agentId`, `result`, `status`, and optional `metrics`.

### chain_subagents (sequential)

Runs subagents sequentially. Each step can reference `{{previous_output}}` from the prior step.

Parameters:

- `steps`: array of step objects
  - `prompt` (string, optional): task for this step
  - `parallel` (array, optional): run multiple tasks in parallel for this step
  - `type` (string, optional): `explore`, `plan`, or `general` (default `explore`)
  - `thoroughness` (string, optional): `quick`, `medium`, or `thorough` (default `medium`)
  - `resume` (string, optional): resume a prior subagent by agentId

Returns: `final_result` (string) and a `chain` array with step outputs.

## Subagent types

- `explore`: read-only search and analysis with restricted tools.
- `plan`: read-only planning-focused investigation.
- `general`: full read/write tasks (cannot spawn subagents).

All subagents use `gpt-5.1-codex-mini` with reasoning effort `medium`.

## Safety and limits

- No recursion: subagents cannot spawn other subagents.
- Concurrency limit: max 24 subagents at once.
- Timeout: 5 minutes per subagent.
- Read-only subagents restrict tool access and shell commands.

## Transcripts and resume

Each subagent run writes a transcript to:

```
~/.codex/agents/agent-{agentId}.jsonl
```

Pass `resume` with an existing `agentId` to continue from prior context.
