# Reflection Layer

The reflection layer is an experimental feature that verifies if the AI agent completed a task correctly by using a "judge" model to evaluate the work.

## How It Works

1. **Task Execution**: Agent executes the user's request using tools (shell, file operations, etc.)

2. **Evaluation**: After completion, the reflection layer:
   - Collects context: original task, recent tool calls (up to 10), and final response
   - Sends this to a judge model for evaluation
   - Receives a verdict with completion status and confidence score

3. **Retry Loop**: If the task is incomplete:
   - Judge provides feedback on what's missing
   - Agent receives feedback and tries again
   - Repeats up to 3 attempts (configurable)

## Verdict Structure

```json
{
  "completed": true,
  "confidence": 0.95,
  "reasoning": "Task was completed successfully",
  "feedback": null
}
```

- `completed`: Whether the task was done
- `confidence`: 0.0 to 1.0 confidence score
- `reasoning`: Explanation of the verdict
- `feedback`: Instructions for the agent if incomplete

## Configuration

Enable in `~/.codex/config.toml`:

```toml
[reflection]
enabled = true
max_attempts = 3

[features]
reflection = true
```

## Running Tests

```shell
# Required environment variables
export AZURE_OPENAI_API_KEY="<key>"
export AZURE_OPENAI_BASE_URL="<url>"

# Optional: specify model (defaults to gpt-5-mini)
export AZURE_OPENAI_MODEL="gpt-5"

# Run the integration test
cargo test -p codex-core --test all --release reflection_layer_hello_world -- --ignored --nocapture
```

The test verifies:
1. Azure OpenAI integration with reflection enabled
2. Agent creates requested Python files
3. Tests pass via pytest
4. Reflection layer evaluates and returns a verdict

## Evaluation Suite (SWE-bench Style)

The eval suite measures the reflection layer's impact on coding task performance, inspired by [SWE-bench](https://github.com/SWE-bench/SWE-bench).

### Tasks

| Task | Description | Bug Type |
|------|-------------|----------|
| Task 1 | Off-by-one errors | `range(n+1)` → `range(n)`, index errors |
| Task 2 | String logic | Palindrome detection, word counting |
| Task 3 | Edge cases | Division by zero, empty list handling |

Each task provides:
- A buggy Python codebase
- An issue description (like a GitHub issue)
- Test files that verify the fix

### Running Evaluations

```shell
# Run single task with reflection
cargo test -p codex-core --test all --release eval_task1_offbyone_with_reflection -- --ignored --nocapture

# Run single task without reflection
cargo test -p codex-core --test all --release eval_task1_offbyone_without_reflection -- --ignored --nocapture

# Run full comparison (all tasks, with and without reflection)
cargo test -p codex-core --test all --release eval_summary -- --ignored --nocapture
```

### Sample Output

```
========================================
SWE-BENCH STYLE EVALUATION SUMMARY
========================================

--- Task 1: Off-by-one errors ---
  With reflection:    PASS (verdicts: 1)
  Without reflection: PASS

--- Task 2: String logic errors ---
  With reflection:    PASS (verdicts: 1)
  Without reflection: PASS

--- Task 3: Missing edge cases ---
  With reflection:    PASS (verdicts: 2)
  Without reflection: FAIL

========================================
RESULTS
========================================
With reflection:    3/3 tasks passed
Without reflection: 2/3 tasks passed
Improvement: +1 tasks
```

The reflection layer helps catch incomplete fixes by re-evaluating the agent's work and providing feedback for another attempt.
## Local Debugging

### Prerequisites

- Rust toolchain with `cargo` installed.
- `just` available for the repo (if you use it for formatting/linting).
- (Optional) `cargo-insta` if you will work with snapshot tests.

> Note: On Windows prefer WSL for these instructions or adapt commands to PowerShell.

### Installation

Preferred (recommended): install the CLI into your local bin with `cargo install`:

```shell
# from repo root
cargo install --path codex-rs --root "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
```

Alternative: build and copy (guarded):

```shell
mkdir -p "$HOME/.local/bin"
cargo build --release
BINARY="codex-rs/target/release/codex"
if [ -f "$BINARY" ]; then
  install -m 755 "$BINARY" "$HOME/.local/bin/"
  echo "Installed codex to $HOME/.local/bin"
else
  echo "Error: built binary not found at $BINARY" >&2
  exit 1
fi
```

Notes:
- Using `install -m 755` sets the executable bit and is safer than `cp`.
- Avoid using `sudo` unless installing to system locations like `/usr/local/bin`.

### Configure Azure OpenAI provider (`$HOME/.codex/config.toml`)

Replace placeholders in the file below with your values. Placeholders are shown in ALL_CAPS and must be replaced.

```toml
# Example config - replace placeholders
model = "gpt-5-mini" # example model; replace if needed
model_provider = "azure"

[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_AZURE_RESOURCE.openai.azure.com/openai" # replace YOUR_AZURE_RESOURCE
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
request_max_retries = 3
stream_max_retries = 3
stream_idle_timeout_ms = 120000

[model_providers.azure.query_params]
api-version = "2025-04-01-preview"

[reflection]
enabled = true
model = "gpt-5-mini"
max_attempts = 3
```

Important: `base_url` must match your Azure endpoint; e.g. `https://myresource.openai.azure.com/openai`. The `model` value is illustrative — ensure the model is available for your provider/account.

### Environment variables

Add to your shell rc (e.g. `$HOME/.zshrc` or `$HOME/.bashrc`):

```shell
export PATH="$HOME/.local/bin:$PATH"
export AZURE_OPENAI_API_KEY="YOUR_API_KEY" # do not commit this to version control
```

After editing, run `source "$HOME/.zshrc"` or open a new shell.

### JSON vs TOML config

If you previously used `$HOME/.codex/config.json`, be aware that JSON config may override TOML. To keep a backup and avoid destructive moves:

```shell
if [ -f "$HOME/.codex/config.json" ]; then
  cp "$HOME/.codex/config.json" "$HOME/.codex/config.json.bak"
  echo "Backed up existing JSON config to $HOME/.codex/config.json.bak"
fi
```

### Sandbox / Network note

If you run in a restricted (sandboxed) environment, features requiring outgoing network connections may not work. Check environment variables such as `CODEX_SANDBOX_NETWORK_DISABLED` or run with a network-enabled environment if needed.

### Testing

Interactive:

```shell
# run the binary (interactive)
codex
```

Non-interactive (example):

```shell
codex exec --full-auto "Create a Python hello world program"
# Verify reflection: look for "Reflection verdict" (case-insensitive)
codex exec --full-auto "Create test.py that prints 'hello'" 2>&1 | grep -i "Reflection verdict" || true
```

### Running Unit Tests

Run crate-specific tests (preferred):

```shell
cargo test -p codex-core reflection
```

Run lib-only with verbose output:

```shell
cargo test -p codex-core --lib reflection -- --nocapture
```

If you changed shared crates (core, protocol), run the full test suite:

```shell
# After local crate tests pass
cargo test --all-features
```

Snapshot tests (if applicable):

- If you update UI/text snapshots in `codex-tui`, follow the repo snapshot flow:
  - `cargo test -p codex-tui`
  - `cargo insta pending-snapshots -p codex-tui`
  - `cargo insta accept -p codex-tui` (only if you intend to accept all new snapshots)

### Formatting / linting for Rust code

After making Rust changes, run:

```shell
# format
(cd codex-rs && just fmt)

# fix lints for the specific project you changed, e.g. codex-core
(cd codex-rs && just fix -p codex-core)
```
