# Streaming stdout and stderr from long-running processes

## https://github.com/openai/codex/issues/4751

What feature would you like to see?
I would like the ability to see the real-time stdout and stderr from long-running shell commands that Codex executes, especially those run via MCP tools.

Currently, when Codex runs a command like npm run test:e2e or a large npm install, the TUI only displays a generic spinner (e.g., ⠏ Running npm run test:e2e). This provides no visibility into the progress, output, or potential errors of the underlying process.

The ideal behavior would be for the output of that command to be streamed directly into the TUI, below the "Running..." indicator. This would allow users to:

Monitor the progress of the task (e.g., see test suites running, files being compiled).
Debug issues in real-time if the command produces errors.
Know that the process has not hung or stalled.
This would significantly improve the user experience and build confidence that Codex is actively working on the task.

Additional information
I encountered this issue while using the Playwright MCP to run end-to-end tests on a project. Codex correctly identified the task, created a plan, installed dependencies, and then initiated the test command: npm run test:e2e.

At this point, the UI showed a spinner and the message ⠏ Running npm run test:e2e for over 20 minutes with no further updates or information. I had no way of knowing if the tests were running, stalled, or had failed. The Playwright test runner typically has a rich, detailed output showing which tests are running, passing, or failing, all of which was completely hidden.

While my specific case was with Playwright, this feature is essential for any potentially long-running process, such as:

Running database migrations (prisma migrate dev).
Complex build scripts (npm run build).
Large dependency installations (npm install).
Providing this visibility is critical for a tool designed for developers.

TUI log showing the stall point:

```
› use the playwright mcp to extensively test this project. make a plan first.

• Updated Plan
  └ □ Review available Playwright tooling and existing test structure to understand required setup.
    □ Install dependencies and ensure test environment prerequisites are satisfied.
    □ Execute Playwright end-to-end suites via MCP integration with appropriate configurations.
    □ Summarize results and note any observed failures or follow-ups.

• Explored
  └ List e2e
    List ls
    Read playwright.config.ts, playwright-global-setup.ts
    Search MCP

• Updated Plan
  └ ✔ Review available Playwright tooling and existing test structure to understand required setup.
    □ Install dependencies and ensure test environment prerequisites are satisfied.
    □ Execute Playwright end-to-end suites via MCP integration with appropriate configurations.
    □ Summarize results and note any observed failures or follow-ups.

• Explored
  └ Read env.setup.js
    List ls -a
    Read .env.test
    List supabase
    Read package.json

• Ran node -v
  └ v24.8.0

• Ran npm install
  └
    added 14 packages, and audited 1113 packages in 2s
    … +3 lines

    found 0 vulnerabilities

• Explored
  └ Read .env

• I’m considering the implications of running tests that connect to a remote Supabase instance using provided
  credentials, which might modify real data. I want to ensure safe setup by verifying connectivity without
  destructive actions and confirming necessary dependencies like Playwright are installed before proceeding.

• Ran npx playwright install
  └ (no output)

• Updated Plan
  └ ✔ Review available Playwright tooling and existing test structure to understand required setup.
    ✔ Install dependencies and ensure test environment prerequisites are satisfied.
    □ Execute Playwright end-to-end suites via MCP integration with appropriate configurations.
    □ Summarize results and note any observed failures or follow-ups.

• Explored
  └ Read video-features-mcp-headless.spec.ts

• I’m making sure the app server and Supabase environment are properly set up for running npm run test:e2e. The
  tests rely on remote Supabase for login and API routes, so environment variables from .env must be loaded. I'll
  run all tests across browsers, noting they might be time-consuming but should pass since some tests auto-pass
  while others expect manual steps.

⠏ Running npm run test:e2e

• Preparing to run Playwright tests (23m 49s • esc to interrupt)
```

### Follow-up comment [to 4751](https://github.com/openai/codex/issues/4751#issuecomment-3368703897)

Here is a potential high-level architectural plan to implement real-time output streaming for shell commands in the Codex TUI.

Architectural Plan: Real-Time Command Output Streaming
1. Guiding Principles
Decoupling: The solution should be layered. The command execution logic should not know about the TUI's rendering details. Communication should happen via a well-defined event protocol.
Generality: The streaming mechanism should work for both local shell commands (local_shell) and commands executed via MCP tools. The TUI should not need to differentiate between them.
Efficiency: We must stream output in chunks without waiting for the command to complete, but also without overwhelming the TUI with too many render updates.
2. High-Level Overview
The core idea is to change the command execution flow from a synchronous "request-response" model (where the full output is returned at the end) to an asynchronous, event-driven streaming model.

This involves three main areas of work:

Core Execution Layer: Modify the command execution logic to capture and stream stdout and stderr chunks in real-time.
Protocol Layer: Introduce a new event type to carry these real-time output chunks from the execution layer back to the UI.
TUI Presentation Layer: Update the TUI's history and active command cells to receive and render these streaming updates.
3. Detailed Component Breakdown
Component 1: Core Execution Layer (codex-core)
This is where the command process is spawned and its output is read.

Introduce a New Protocol Event:

In codex-rs/core/src/protocol/protocol.rs, we will define a new event, ExecCommandOutputDeltaEvent.
This event will contain:
call_id: The unique identifier for the specific tool call, allowing the TUI to associate the output with the correct running command.
stream: An enum indicating the source (stdout or stderr).
data: The chunk of output as a String or Vec<u8>.
Modify the Execution Pipeline:

The current function process_exec_tool_call in codex-rs/core/src/exec.rs waits for the command to finish and returns a complete ExecToolCallOutput. This needs to be adapted.
We will introduce an optional event_sender (a tokio::sync::mpsc::Sender<Event>) into the execution context, likely passed into process_exec_tool_call via a new field in the StdoutStream struct.
The read_capped function, which already reads from the child process's streams in a separate task, is the ideal place to implement the streaming. Instead of just appending to a buffer, it will now also send ExecCommandOutputDeltaEvents through the provided channel for each chunk it reads.
Update Final Output:

The final ExecCommandEndEvent will still be sent when the command completes. Its payload (stdout, stderr, aggregated_output) will contain the full captured output, ensuring that even if the TUI misses some delta events (e.g., due to high volume), the final state is consistent. The live streaming is for UX, while the final event is for the permanent record.
Component 2: TUI Presentation Layer (codex-tui)
This layer will receive the new events and render the live output.

Handle the New Event:

In codex-rs/tui/src/chatwidget.rs, the main event loop (ChatWidget::handle_codex_event) will get a new case for EventMsg::ExecCommandOutputDelta.
Update the Active Command Cell Model (ExecCell):

The ExecCell struct in codex-rs/tui/src/exec_cell/model.rs, which represents a command execution in the UI, currently only holds the final CommandOutput.
We will add a new field to ExecCell to buffer the live, streamed output lines, for example: live_output: Vec<Line<'static>>. This buffer will be populated as ExecCommandOutputDeltaEvents arrive.
Enhance Rendering Logic:

The rendering logic in codex-rs/tui/src/exec_cell/render.rs for an active ExecCell (one that is still running) will be updated.
Currently, it just shows a spinner and the command. The new logic will render the contents of the live_output buffer directly below the command line, effectively creating a mini-terminal view for the running command.
We will implement a capped-size circular buffer for live_output to prevent memory issues from extremely verbose commands, showing only the most recent N lines.
Event Flow in the TUI:

When ExecCommandBeginEvent arrives, an ExecCell is created and stored as the active_cell in ChatWidget.
As ExecCommandOutputDeltaEvent events arrive, ChatWidget finds the active_cell, downcasts it to ExecCell, and appends the new output lines to its live_output buffer. It then requests a TUI redraw.
When ExecCommandEndEvent arrives, the active_cell is finalized with the full output and exit code, and it is moved from the active_cell slot into the permanent history log (transcript_cells).
Component 3: MCP Tool Integration (Generalization)
To support tools like Playwright running over MCP, the streaming mechanism must be extended.

MCP Server (codex-mcp-server):

The codex_tool_runner which spawns and manages the CodexConversation for a tools/call request will listen for the new ExecCommandOutputDeltaEvent.
Upon receiving this event, it will forward the data to the connected MCP client using an appropriate MCP notification, likely a tool/stream notification type if the spec supports it, or a custom codex/exec_output_delta notification.
MCP Client (codex-core):

The McpConnectionManager will be updated to listen for these new streaming notifications from the server.
When it receives a streaming notification, it will transform it back into a local ExecCommandOutputDeltaEvent and send it to the Session's event queue (tx_event).
Result: From the TUI's perspective, there is no difference. It receives an ExecCommandOutputDeltaEvent regardless of whether the command was executed by the local shell or a remote MCP server, achieving the desired generality.

4. Implementation Stages
Protocol First: Define ExecCommandOutputDeltaEvent in codex-protocol and codex-core.
Local Shell Implementation: Implement the streaming logic for local commands first. This involves modifying codex-core/src/exec.rs and the TUI's ExecCell to handle and display the new event. This provides the core functionality quickly.
TUI Presentation: Update the ChatWidget and ExecCell to render the streamed output, including handling line wrapping and scrolling within the active cell.
MCP Extension: Extend the functionality to the MCP server and client layers to enable streaming for all tool types.
This plan establishes a robust pipeline for real-time data, significantly enhancing the user experience by providing the requested visibility into long-running commands.

## https://github.com/openai/codex/issues/3675

What feature would you like to see?
Show stdout/stderr of commands run by the shell tool. Its a lot easier to get an idea of what an LLM is doing behind the scenes that way.

Would be great if it's implemented kind of like how claude code does it (truncated by default but you can press Ctrl + R to expand). I wouldn't even mind not having the expand features just a greyed out truncated output is good enough.

## https://github.com/openai/codex/issues/4179

What feature would you like to see?
Summary

Add a non-interactive, headless “supervise & repair” mode to Codex CLI that continuously stabilizes a running app by orchestrating Playwright MCP to simulate real users, watching live logs, auto-diagnosing failures, applying fixes, and resuming the same user journey—repeating until a configurable zero-error stability goal is met (e.g., no critical errors for N minutes).

Problem

Long-running processes (dev servers, Docker) and evolving features often break user flows. Codex currently tends to act after a command finishes, which doesn’t work for never-ending tasks and delays remediation. We need an automatic, under-the-hood loop that observes the app while it runs, drives a real browser like a user, fixes issues the moment they appear, and continues without human intervention.

Desired Behavior

Start or attach to the app process and stream stdout/stderr incrementally (no wait for process exit).

Connect to Playwright MCP and execute live user flows (navigate, type, click, submit, validate).

On any runtime failure (console errors, unhandled exceptions, HTTP 5xx/4xx thresholds, Playwright selector/timeouts, health-check failures), Codex:

Captures artifacts (logs, screenshot, trace, DOM snapshot, network/HAR).

Drafts a root-cause and a minimal, safe code change.

Applies the fix automatically (policy-gated, no prompts).

Hot-reloads or restarts the affected service if needed.

Resumes the exact flow at the failing step.

Repeats until stability goals are met or bounded retry/backoff limits are reached.

Scope

Automation only: runs headless; no UI/TUI.

Playwright MCP as the browser driver; optional direct HTTP calls (via MCP tool) when UI interaction isn’t necessary.

Supports multiple supervised processes (e.g., web, API, worker).

Design Requirements

Live log ingestion: unbuffered, partial-line reads with lightweight summarization and debounced trigger evaluation.

Trigger model: regex patterns, severity/keyword thresholds, JSON-structured log queries, and MCP/Playwright events (timeouts, “not found,” navigation errors).

Repair policy: apply only minimal, reversible diffs; keep a rollback plan; bound changes per cycle (e.g., max lines changed/files touched).

Resume semantics: restart from the same Playwright step with state restoration (storage state, cookies, auth), or rerun the smallest stable sub-flow.

Retry/backoff: progressive delays, circuit-breaker after repeated failure modes, clear stop conditions.

Stability goals: configurable targets (e.g., “zero critical errors for 5 minutes,” “no console.error above allowlist,” “network-healthy: 0% 5xx and <N% 4xx during window”).

Non-interactive: never pause to ask; if a fix exceeds policy limits, halt with a concise remediation summary and artifact bundle.

Event Sources & Triggers (examples)

Dev/app/Docker logs; console.error; unhandled exceptions and stack traces.

Playwright events: selector not found, timeouts, navigation failures.

Network signals: HTTP 5xx, elevated 4xx, broken CORS/preflight, auth expiry.

Health-checks and readiness probes.

Custom allow/deny patterns per project.

Repair Workflow (automated)

Snapshot: sliding window of logs; screenshot/video; DOM snapshot; network trace/HAR; key metrics.

Root-cause draft: likely file/line, failing selector/route, minimal repro.

Diff: smallest change to restore correctness (e.g., locator tweak, null-guard, missing import, mis-ordered await, route handler fix).

Apply: write change, run formatter/linter, trigger rebuild/hot-reload or restart with backoff.

Resume: continue same flow; on failure, retry with bounds, then escalate/halt.

Safety & Controls

Bounded blast radius: caps on lines changed, files touched, elapsed change time.

Rollback: atomic diffs and easy revert; keep a per-cycle artifact/diff trail.

Secrets & privacy: redact tokens, cookies, and auth headers by default; configurable scrub rules.

Rate limiting: protect external APIs during automated retries.

Observability (headless)

Structured logs describing: triggers fired, diffs applied, restart events, retry counts, and stability status.

Artifact bundle per cycle stored locally (screenshots, traces, HAR, logs, diffs).

Platform & MCP

Robust Playwright MCP session management (startup, health checks, retries) on macOS, Linux, and Windows/WSL.

Auto-detection of browsers/deps with clear remediation hints (text logs only).

Acceptance Criteria

Incremental log streaming and trigger detection while the app runs (no reliance on process exit).

Reliable Playwright MCP control with automatic resume at the failing step after each fix.

Convergence to a configurable stability goal (e.g., zero critical errors for N minutes) without human interaction.

Graceful handling of non-terminating processes (attach/detach, restart with backoff).

Per-cycle artifact and diff output suitable for review or CI follow-up.

Open Questions

Default allow/deny lists for console/network patterns across common stacks?

Heuristics for what qualifies as a safe, auto-applicable diff vs. a halt-and-report?

Optional CI mode that reuses the same non-interactive loop for pre-merge stabilization?

## https://github.com/openai/codex/issues/4550

Problem
Currently, the Codex CLI TUI forcibly folds multi-line commands and command output, showing only a few lines and replacing the rest with an ellipsis line (e.g., … +X lines). This makes it very difficult to review, debug, or copy full scripts or command outputs generated by the model, especially for heredoc-heavy workflows (e.g., python3 - <<'PY' ...).

There is no user-exposed setting in the CLI or config file to disable or configure this folding/truncation. The behavior is hard-coded in the TUI. Many users would prefer to see the full command/output or at least have the ability to customize the folding threshold.

References
Folding/truncation logic: codex-rs/tui/src/exec_cell/render.rs
This is not a terminal feature; it's implemented by Codex CLI itself.
Requested Solution
Add a user-facing configuration option (CLI flag or config.toml) to:
Disable command folding/output truncation entirely (always show full content)
Or, allow customizing the number of lines shown before folding (both for command lines and output)
Optionally, provide a per-session or per-command override (e.g., a hotkey to expand/collapse in the TUI)
Workarounds (not sufficient)
Using headless/non-interactive mode avoids the TUI, but disables the interactive session.
Building from source to raise caps is not user-friendly.
Impact
Users running or reviewing long scripts (e.g., model-generated bash/python heredocs) can see the full command, copy/edit/debug as needed.
Makes Codex CLI more transparent and user-friendly for advanced workflows.
Filed by @Konjac-XZ. If more details are needed, please let me know.
