# Android Agent + Genie Refactor

## Status

This document tracks the Android Agent Platform refactor that moves Codex from a
single Android wrapper process to a framework-native Agent/Genie architecture.

The current repo now contains these implementation slices:

- `android/app` is a buildable **Agent** app scaffold.
- `android/genie` is a buildable **Genie** app scaffold.
- The Agent app can:
  - register `AgentService`
  - plan target packages for a user objective
  - bridge framework session APIs into the hosted Agent Codex runtime
  - launch direct parent + child framework sessions
  - start one Genie session per selected target package
  - display framework session state and event timelines
  - answer waiting Genie questions
  - attach detached targets
- The Genie app now hosts a real `codex app-server` subprocess, packaged inside
  the Genie APK as `libcodex.so`.
- The first internal Agent<->Genie control plane uses the framework-managed
  **per-session bridge** returned by `AgentManager.openSessionBridge(...)` on
  the Agent side and `GenieService.Callback.openSessionBridge(...)` on the
  Genie side, not framework question/answer events.
- The older direct cross-app bind/socket bridge experiments have been removed
  from the app code; the framework session bridge is now the only supported
  Agent<->Genie control plane.
- The current session bridge exposes small fixed-form calls, and the Genie
  runtime already uses it to fetch Agent-owned runtime metadata from the
  hosted Agent Codex runtime, including auth status and configured model/provider.
- The Android host layer now copies a checked-in `AGENTS.md` asset into the
  Agent Codex home at bootstrap, and each Genie session copies that installed
  Agent file into its per-session Codex home over the framework session bridge.
- Target-package planning now relies on the hosted Agent Codex runtime using
  standard Android shell tools already available on-device (`cmd package`, `pm`,
  `am`) instead of Kotlin-side app discovery wrappers.
- Direct child-session launch now runs through a dedicated hosted Agent
  framework-session bridge, with Kotlin reduced to the framework/session host
  layer.
- The Genie runtime now relies on the hosted Codex shell/tool path for target
  package inspection, activity launch, input injection, and UI dumping instead
  of host-side Kotlin wrappers for those operations.
- The hosted `codex app-server` process now routes `/v1/responses` traffic over
  the existing app-server JSON-RPC channel to the Android host, and the Genie
  host now executes those requests through the framework-owned HTTP session
  bridge instead of bouncing them through the Agent app UID.
- The Agent now provisions per-session network config with auth/base-url
  inputs before each Genie start, while the framework owns active session
  `/responses` execution and streaming.
- Agent-anchored planning now uses one hosted Agent runtime per planning
  session instead of a single shared planner runtime, so multiple top-level
  Agent sessions can plan concurrently.
- Planner runtime ownership is 1:1 with the top-level parent session id. The
  app does not allow overlapping planner runtimes for the same parent session.
- The framework now permits both active Genie runtimes and active top-level
  Agent runtimes to open framework HTTP exchanges. Direct parent planner
  runtimes therefore use the same framework-owned `/responses` transport as
  Genie child runtimes, and the old Agent-owned planner fallback has been
  removed.
- The Genie runtime now keeps host dynamic tools limited to framework-only
  detached-target controls and frame capture, while standard Android shell and
  device commands stay in the normal Codex tool path.
- Non-bridge Genie questions surface through AgentSDK question flow by mapping
  `request_user_input` back into Agent-managed questions and answers.
- Genie-hosted app-server sessions now explicitly enable
  `request_user_input` in Default mode so ambiguous delegated tasks can pause in
  real framework `WAITING_FOR_USER` state instead of guessing or emitting a
  plain-text question as a normal completion.
- In detached mode, Genie question handoff no longer changes the current target
  presentation. A hidden session therefore stays `DETACHED_HIDDEN` while waiting
  for user input unless Codex explicitly asks to show or attach the target via a
  framework target-control tool.
- The Agent now auto-answers only internal bridge questions. User-facing Genie
  questions remain user-facing and are surfaced through the normal framework
  question flow, desktop/UI answer surfaces, and framework-delegated
  AGENT-rendered notifications with target-app icon branding and inline reply.
- Framework-owned notification lifecycle now delegates question/result/error
  notification rendering to the Agent app when available, while retaining a
  framework fallback path if the Agent renderer is unavailable or does not ACK
  in time. Inline reply actions route through
  `answerQuestionFromNotification(sessionId, notificationToken, response)`.
  Running-session callbacks are ACKed but intentionally suppressed so the Agent
  only notifies when user input is needed or a terminal outcome is ready.
- The Agent now records an explicit per-child final presentation policy
  (`ATTACHED`, `DETACHED_HIDDEN`, `DETACHED_SHOWN`, or `AGENT_CHOICE`) and
  uses the framework-authoritative `AgentSessionInfo.getTargetPresentation()`
  state to verify whether a completed child actually satisfied it.
- Target visibility is now hidden-by-default. The planner should prefer
  `DETACHED_HIDDEN` unless the user explicitly asks for a visible app handoff
  or the task clearly implies that the final UI must be shown, and Genie
  runtime instructions/tool descriptions reinforce that `show` / `attach`
  controls should not be used just for inspection.
- For HOME-anchored app sessions, Launcher now standardizes badged icon taps as
  AGENT dispatch for `RUNNING`, `WAITING_FOR_USER`, and `COMPLETED` states. The
  Agent popup therefore owns the tap policy explicitly:
  - for running HOME states with a detached target, the popup activity
    immediately calls `showDetachedTarget(...)` so a red-badged live icon brings
    the paired app on screen without attaching it and terminating the Genie
    session
  - for question states, the popup shows a shared reply composer with the target
    app icon, question text, and Cancel / Answer actions
  - for result states, the same popup shape shows the final message, an optional
    follow-up composer, and OK / Send actions
  - pressing OK consumes the HOME badge/live-tile presentation through
    `consumeHomeSessionPresentation(sessionId)` and closes the detached target if
    one is still present instead of relying on Launcher to consume or open the
    target app directly
  - pressing Send on a HOME result starts a fresh app-scoped continuation session
    with previous-result context, then consumes the completed HOME presentation
  - pressing Send on an AGENT result continues the direct parent session in place
- Direct `ANCHOR_AGENT` parent sessions can now surface on HOME through the
  framework-derived parent-session icon surface:
  - Codex exposes two launcher entrypoints: `Codex Manager` for the
    administrative session list/debugging UI and `Codex` for the direct
    planner-session prompt flow
  - the `Codex` launcher entrypoint opens only the planner-session prompt UI;
    it does not route through the management list
  - parent HOME icon taps route through `HANDLE_SESSION` and then into the same
    popup/detail flow, with question states preferring the waiting child
    question when the parent is the requested session
  - pressing `Done` on a direct-parent result consumes only the parent HOME icon
    via `consumeHomeSessionPresentation(parentSessionId)` and leaves the parent
    session inspectable in `Codex Manager`
- Codex Agent still uses `cancelSession(sessionId)` for user-driven cancellation
  because it is the AGENT-role app, not a HOME-role surface. The
  HOME-only `cancelHomeSession(sessionId)` API is reserved for Launcher/HOME
  callers.
- Parent roll-up now uses the new presentation state. If a child was required
  to finish `ATTACHED` but completes detached, the Agent requests `attachTarget`
  before rolling the parent up to success. Detached shown/hidden mismatches are
  treated as real completion errors instead of silent success.
- Runtime testing on the emulator showed that direct cross-app `bindService`
  and raw local-socket access from the live Genie runtime are not a stable
  contract because the runtime executes under the paired target sandbox
  identity. The framework session bridge is the correct transport boundary.
- Runtime testing on the updated image shows that self-targeted shell activity
  launch is viable from the paired Genie sandbox when the command shape is
  correct, for example `cmd activity start-activity --user 0 ...` or
  `am start --user 0 ...`.
- Detached HOME sessions are a stricter contract: once the framework has
  launched the target hidden, Codex must not relaunch that same target package
  with plain shell launchers. That bypasses detached hosting and can be blocked
  by Android background-activity-launch policy.
- Detached-session relaunch policy violations are now surfaced back into the
  hosted Genie runtime as normal command failures plus framework traces, instead
  of throwing host-side exceptions that automatically fail the session.
- Detached-session state now distinguishes presentation from runtime existence.
  The app consumes `targetRuntime`, typed detached control results, and typed
  capture results so a missing detached target can be restored through
  framework-owned recovery instead of guessed ordinary app launch.
- Recoverable hosted-runtime failures are now intentionally non-terminal when a
  fresh `codex app-server` thread can still be bootstrapped:
  - attached Genie sessions close only the current desktop attach, then restart
    into a fresh attachable idle thread with staged recovery context
  - unattached Genie sessions retry automatically with staged recovery context
    and only pause into an attachable idle recovery thread if automatic recovery
    is exhausted
  - only failures that prevent bootstrapping any new hosted runtime at all
    should still terminate the Genie session

The Android app now owns auth origination, runtime status, and per-session
transport configuration handoff. Active Genie and top-level Agent planner model
traffic are framework-owned.
The older standalone
service/client split has been removed from the repo and is no longer part of
the Android Agent/Genie flow.

## Fixed Architecture Decisions

- There is one **Agent** runtime.
- There is one installed **Genie APK**.
- The framework launches **distinct Genie session instances**, each bound to one
  target app.
- A running Genie session instance is the unit of target pairing, sandboxing,
  and Android tool access.
- Genie is **headless** and must not access the internet directly.
- The Agent<->Genie product contract is free-form text:
  - Agent -> Genie: free-form objective / instructions
  - Genie -> Agent: free-form result / progress / follow-up question
  - Agent decides whether to answer directly or escalate to the user
- The user interacts only with the **Agent**.
- The Agent decides which target package(s) should receive child Genie sessions.
- Each child Genie decides its own local tool usage inside the paired sandbox.
- The Agent is the only runtime that owns:
  - sign-in UX and auth-material origination
  - non-session outbound network access
  - upstream provider selection and session-scoped network config handoff
  - orchestration of parent + child sessions
- Internal Agent<->Genie coordination now splits into:
  - framework per-session bridges for fixed-form control/data RPC
  - AgentSDK session events for free-form product dialogue
- hosted `codex app-server` inside Genie for the actual Codex execution loop
- Genie-local transport termination between the hosted `codex` child process
  and the framework session bridges
- framework-owned Responses transport termination between the live Genie session
  and the upstream model backend

## Runtime Model

### Agent

- Framework-facing Android app with:
  - `AgentService`
  - `HANDLE_SESSION` activity
  - session orchestration via `AgentManager`
- Responsible for:
  - user interaction
  - task planning and target-package selection
  - direct parent session creation
  - child session creation per target app
  - starting Genie sessions
  - answering Genie questions
  - aggregating child progress/results into a parent task
  - provisioning per-session auth/base-url transport inputs for framework-owned
    Genie traffic

### Genie

- Single APK exposing `GenieService`
- Framework launches one Genie execution per target app session
- Responsible for:
  - driving the paired target app
  - publishing trace, question, result, and error events
  - requesting detached target actions when appropriate
- The current implementation hosts `codex app-server` inside both the Agent and Genie sandboxes.
- Agent-side planning no longer uses one shared hosted runtime; each concurrent
  planning session owns its own short-lived hosted `codex app-server`
  subprocess.
- The Genie host now treats detached-target preparation failures, detached
  target relaunch policy violations, and recoverable hosted I/O failures as
  recoverable runtime context whenever possible instead of auto-failing the
  framework session.
- Kotlin is now only the host/bridge layer for:
  - framework lifecycle and result publication
  - Android dynamic tool execution
  - Agent escalation via `request_user_input`
  - runtime bootstrap from the framework session bridge
  - forwarding hosted `codex` `/v1/responses` traffic from both Agent and Genie
    runtimes onto the framework-owned HTTP bridge

## First Milestone Scope

### Included now

- AgentSDK compile-time integration via
  `ANDROID_AGENT_PLATFORM_STUB_SDK_ZIP` or `-PagentPlatformStubSdkZip=...`
- Agent app manifest/service wiring
- Genie app manifest/service wiring
- Direct session launcher in the Agent UI
- Agent-side target-package planning with an optional package override
- Hosted Agent planning via standard Android shell tools already available on-device
- Dedicated framework-session bridge tools for direct Genie-session launch and question resolution
- Framework session inspection UI in the Agent app
- Question answering and detached-target attach controls
- Explicit per-child final target presentation policy in planning/session
  launch, backed by framework-authoritative presentation state in session
  observation and diagnostics
- Framework session bridge request handling in `AgentSessionBridgeServer`
- Framework session bridge request issuance in `CodexGenieService`
- Agent-hosted runtime metadata for Genie bootstrap
- Shell-first Genie execution for package inspection, activity launch, input injection, and UI dumping
- Hosted `codex app-server` inside Genie, with model traffic routed through the
  app-server request/response channel and then through the framework-owned
  streaming HTTP exchange bridge
- Per-session framework transport provisioning in
  `android/bridge/src/main/java/com/openai/codex/bridge/FrameworkSessionTransportCompat.kt`
- Framework-only Android dynamic tools registered on the Genie Codex thread with:
  - detached target ensure-hidden/show/hide/attach/close
  - typed detached frame capture with runtime-aware recovery
- `request_user_input` bridged from hosted Codex back into AgentSDK questions
- Framework-owned notification lifecycle with delegated Agent rendering for
  question/result/error notifications; the Agent app posts the visible
  notification, ACKs the tokenized callback, and falls back to plain
  framework-owned rendering if the new callback surface is unavailable. Running
  progress notifications are suppressed by ACKing the callback and cancelling
  any stale session notification instead of posting a new one.
- Detached-mode Genie sessions preserve their current target presentation when
  entering `WAITING_FOR_USER`; user prompts that ask to keep the app hidden should
  therefore remain hidden while waiting for an answer
- Agent-mediated answers only for internal bridge questions that the user
  should never see as product-facing prompts
- Agent planning can now use `request_user_input` to ask the user clarifying
  questions before launching child Genie sessions
- Abstract-unix-socket support in the legacy Rust bridge via `@name` or
  `abstract:name`, so the compatibility transport can move off app-private
  filesystem sockets when Agent<->Genie traffic is introduced

### Not done yet

- Expanding the framework session bridge beyond the current fixed-form runtime
  bootstrap/status calls
- Adding more Android-native tool surfaces and richer observation types to the
  hosted Genie runtime

## Current Code Layout

- `android/app`
  - Agent scaffold and hosted Agent Codex runtime
- `android/genie`
  - standalone Genie scaffold APK with hosted `codex app-server`
- `android/app/src/main/java/com/openai/codex/agent/CodexAgentService.kt`
  - framework `AgentService`, parent roll-up, bridge auto-answering, and
    delegated notification callbacks
- `android/app/src/main/java/com/openai/codex/agent/AgentSessionController.kt`
  - Agent-side `AgentManager` orchestration helper
- `android/app/src/main/java/com/openai/codex/agent/AgentFrameworkToolBridge.kt`
  - hosted Agent bridge for framework session APIs
- `android/app/src/main/java/com/openai/codex/agent/MainActivity.kt`
  - Agent session UI, Agent clarification dialogs, and Agent-native auth controls
- `android/app/src/main/java/com/openai/codex/agent/SessionDetailActivity.kt`
  - HOME/AGENT session inspection and question answering UI
- `android/app/src/main/java/com/openai/codex/agent/SessionPopupActivity.kt`
  - dialog-style question/result surfaces for HOME icon and notification
    entrypoints; question cancel only dismisses the dialog, while final results
    render in a scrollable text view with an OK button that clears HOME
    presentation state for top-level HOME sessions
- `android/app/src/main/java/com/openai/codex/agent/AgentQuestionNotifier.kt`
  - token-aware Agent-side renderer for delegated framework notifications,
    including target-app branding and inline reply actions
- `android/app/src/main/java/com/openai/codex/agent/AgentNotificationReplyReceiver.kt`
  - inline reply receiver for Agent-rendered question notifications
- `android/app/src/main/java/com/openai/codex/agent/AgentUserInputPrompter.kt`
  - Android dialog bridge for hosted Agent `request_user_input` calls
- `android/genie/src/main/java/com/openai/codex/genie/CodexGenieService.kt`
  - Genie lifecycle host for the embedded `codex app-server`
- `android/genie/src/main/java/com/openai/codex/genie/CodexAppServerHost.kt`
  - stdio JSON-RPC host for `codex app-server`, framework-only dynamic tools,
    `request_user_input` bridging, and `/v1/responses` forwarding onto the
    framework-owned HTTP bridge
- `android/genie/src/main/java/com/openai/codex/genie/DetachedSessionCommandShims.kt`
  - detached-session shell shims that turn forbidden target relaunch commands
    into in-band command failures the hosted Genie runtime can recover from
- `android/app/src/main/java/com/openai/codex/agent/AgentSessionBridgeServer.kt`
  - Agent-side server for the framework-managed per-session bridge
- `android/app/src/main/java/com/openai/codex/agent/AgentResponsesProxy.kt`
  - Agent/Genie bridge helper for framework-owned Responses transport setup and
    execution
- `android/genie/src/main/java/com/openai/codex/genie/AgentBridgeClient.kt`
  - Genie-side client for the framework-managed control bridge plus the
    framework-owned streaming HTTP exchange bridge
- `android/app/src/main/java/com/openai/codex/agent/AgentPlannerRuntimeManager.kt`
  - per-planning-session hosted Agent runtime manager, allowing multiple
    concurrent top-level Agent planning sessions
- `android/app/src/main/java/com/openai/codex/agent/AgentCodexAppServerClient.kt`
  - hosted Agent `codex app-server` client for runtime metadata, auth/model
    operations, auto-answering, and narrow Agent tool calls outside the
    concurrent planner runtime path

## Build

Set the Agent Platform stub SDK zip path:

```bash
export ANDROID_AGENT_PLATFORM_STUB_SDK_ZIP=/path/to/android-agent-platform-stub-sdk.zip
```

Build both Android apps:

```bash
cd android
./gradlew :genie:assembleDebug :app:assembleDebug
```

The Android Gradle build depends on `just android-build` for the packaged
`codex` JNI binaries and will run it automatically when the native artifact is
out of date.

## Next Implementation Steps

1. Route more Agent orchestration through the hosted runtime while keeping the
   host bridge limited to framework session APIs.
2. Add more Android-native tool surfaces and richer observation types to the
   hosted Genie runtime.
3. Later, replace the framework-session host bridge with command-line
   equivalents once those are stable enough to rely on.
