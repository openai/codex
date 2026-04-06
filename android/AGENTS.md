# Android Agent/Genie

This file applies to the Android subtree under `android/`. It is developer-facing
context for working on the Android Agent/Genie implementation in this repo.

Do not confuse this with the runtime guidance asset at
`android/bridge/src/main/assets/AGENTS.md`, which is copied into Codex homes on
device for live Agent/Genie sessions.

## Module layout

- `android/app`: Agent app
- `android/genie`: Genie app
- `android/bridge`: shared Android bridge/runtime compatibility layer
- `android/build-agent-genie-apks.sh`: helper for building both APKs
- `android/install-and-provision-agent-genie.sh`: helper for adb install, role
  assignment, and auth seeding

## Default SDK input

When building the Android APKs, use the branch-local Android Agent Platform stub
SDK at:

`$HOME/code/io/ci/aosp/artifacts/aosp/android16-qpr2-release/android-agent-platform-stub-sdk.zip`

The Android build already accepts this through either:

- environment: `ANDROID_AGENT_PLATFORM_STUB_SDK_ZIP`
- script flag: `android/build-agent-genie-apks.sh --agent-sdk-zip ...`
- Gradle property: `-PagentPlatformStubSdkZip=...`

Treat this path as the default SDK source unless the user explicitly says
otherwise.

## Authoritative design references

Read these first when recovering Android Agent/Genie context:

- local architecture/status doc:
  `docs/android-agent-genie-refactor.md`
- SDK docs inside the stub SDK zip:
  - `README.md`
  - `AGENT_GENIE_DESIGN.md`
  - `CONSUMER_GUIDE.md`

Useful inspection commands:

```bash
unzip -p "$HOME/code/io/ci/aosp/artifacts/aosp/android16-qpr2-release/android-agent-platform-stub-sdk.zip" README.md | sed -n '1,220p'
unzip -p "$HOME/code/io/ci/aosp/artifacts/aosp/android16-qpr2-release/android-agent-platform-stub-sdk.zip" AGENT_GENIE_DESIGN.md | sed -n '1,260p'
unzip -p "$HOME/code/io/ci/aosp/artifacts/aosp/android16-qpr2-release/android-agent-platform-stub-sdk.zip" CONSUMER_GUIDE.md | sed -n '1,260p'
```

## Key platform contract to preserve

The current Android work in this repo assumes the same contract described by the
stub SDK docs and the local refactor doc:

- Agent and Genie are separate APKs.
- The framework-managed per-session bridge is the app-private Agent<->Genie
  control plane.
- The framework-owned streaming HTTP exchange is the transport for active
  `/responses` traffic in both top-level Agent planner sessions and live Genie
  child sessions.
- Genie is headless and should not depend on direct internet access.
- Detached target handling must use framework-authoritative presentation/runtime
  state and typed detached-target recovery APIs rather than guessed relaunches.
- Genies should keep the paired app hidden by default. Prefer
  `DETACHED_HIDDEN` unless the user explicitly asks to bring the app to the
  front, asks to leave it visibly shown, or the task clearly implies a visible
  app handoff.
- App-scoped HOME drafts are real framework `STATE_CREATED` sessions created
  before `startGenieSession(...)`; if you expose that flow outside the on-device
  UI, remember that provisional HOME sessions are expected to hold a
  session-UI lease until they are started or cancelled.
- Desktop draft attach is now implemented by bootstrapping an idle app-server
  runtime before the first turn:
  - HOME drafts start Genie with an internal idle-bootstrap sentinel and become
    attachable immediately after bridge bootstrap.
  - direct AGENT drafts spin up an idle planner app-server host inside the
    Agent process.
  - the first prompt can then be typed in the attached desktop TUI instead of
    being supplied to `sessions start`.
- Attached runtime completion semantics are intentionally non-terminal:
  - attached Genie turns remain live after `turn/completed` so the same desktop
    TUI can send follow-up prompts.
  - attached direct AGENT planner sessions stay live after planning completes.
  - child Genie sessions spawned by an attached planner are launched in idle
    desktop-attach mode instead of immediately consuming their delegated prompt.
  - those idle child sessions still receive Agent-provisioned bridge state
    first, stage the delegated objective as runtime context, and remain
    attachable while the planner stays attached.
  - if the planner detaches before the user manually starts the child, the
    staged delegated objective is released automatically as a fallback.
  - after a child turn completes, planner-held child sessions remain attachable
    until the planner attach detaches.
  - once the planner detaches, those held child sessions are allowed to settle
    to their terminal framework state and the parent roll-up can complete.
- Recoverable hosted-runtime failures are also intentionally non-terminal when a
  fresh app-server thread can still be bootstrapped:
  - recoverable app-server / bridge I/O failures during an attached Genie turn
    close only the current desktop attach, then restart the Genie into a fresh
    attachable idle thread with staged recovery context
  - recoverable I/O failures during an unattached Genie run first retry
    automatically with staged recovery context, then pause into an attachable
    idle recovery thread if automatic retries are exhausted
  - only failures that prevent bootstrapping any new hosted runtime at all
    should still terminate the Genie session
- Parent-session cancellation is tree-scoped for direct AGENT sessions:
  cancelling the parent from the desktop bridge, framework tool bridge, or the
  detail UI must cancel the parent and all child Genie sessions through the
  framework `cancelSession(...)` path, even when some of those sessions are
  already terminal.
- Framework-owned session notifications now support delegated AGENT rendering:
  - user-facing question/result/error notifications should be rendered by the
    Agent app when the framework calls `onShowOrUpdateSessionNotification(...)`
    and cancelled when it calls `onCancelSessionNotification(...)`
  - child Genie notification callbacks under a direct AGENT parent should be
    ACKed but not rendered; the parent session icon/notification is the only
    user-facing notification surface for Agent-anchored work
  - RUNNING/CREATED/QUEUED callbacks should be ACKed but suppressed so the
    Agent only notifies for user action or terminal outcomes
  - the Agent must ACK a posted notification with `ackSessionNotification(...)`
    and route inline replies through `answerQuestionFromNotification(...)`
  - if delegated rendering is unavailable, the framework may post a generic
    fallback notification, so app-side notification code must remain
    token-aware and idempotent
- HOME icon / notification taps for question or final-result states should route
  to `SessionPopupActivity`, which uses one dialog-style popup shape for both
  question answering and result follow-up.
  For top-level HOME `RUNNING` states with a detached target, the same handler
  should immediately call `showDetachedTarget(sessionId)` so a red-badged live
  icon brings the paired app on screen without attaching and ending the Genie
  session.
  Launcher may dispatch HOME-anchored icon taps through either
  `ACTION_HANDLE_AGENT_SESSION` or `ACTION_HANDLE_HOME_AGENT_SESSION`; both
  should resolve to the same Agent-owned popup flow.
  The full `SessionDetailActivity` remains the fallback inspector for non-popup
  states and Agent-app initiated navigation.
- HOME completion and cancellation are AGENT-owned policy decisions:
  - pressing OK in the final result popup should call
    `consumeHomeSessionPresentation(sessionId)` and close a still-detached
    target instead of relying on Launcher to consume/open the target directly
  - pressing Send in the final result popup for a HOME session should launch a
    fresh HOME continuation with previous-result context, then consume the old
    result presentation
  - pressing Send in the final result popup for an AGENT session should continue
    the direct parent session in place
  - Codex Agent is an AGENT-role app, not a HOME-role surface, so its
    user-driven cancellation flows should still call `cancelSession(sessionId)`;
    `cancelHomeSession(sessionId)` is for Launcher/HOME callers
- Direct AGENT parent sessions may also surface on HOME:
  - `Codex` should be a launcher entrypoint that opens only the planner-session
    prompt flow
  - `Codex Manager` should remain the explicit admin/session-list entrypoint
  - parent HOME icon taps should route through `HANDLE_SESSION` to the
    per-session popup/detail flow rather than the management list
  - parent result `Done` should call
    `consumeHomeSessionPresentation(parentSessionId)` and leave the session
    inspectable in Manager

## External reference implementations

There are standalone stub apps outside this repo that are useful for
understanding the intended Android API usage:

- Agent stub root:
  `$HOME/code/omix/AgentStub`
- Genie stub root:
  `$HOME/code/omix/GenieStub`

Especially useful files:

- `$HOME/code/omix/AgentStub/src/com/example/agentstub/ValidationAgentService.java`
- `$HOME/code/omix/AgentStub/src/com/example/agentstub/AgentOrchestrationService.java`
- `$HOME/code/omix/AgentStub/src/com/example/agentstub/SessionActivity.java`
- `$HOME/code/omix/AgentStub/README-standalone.md`
- `$HOME/code/omix/GenieStub/src/com/example/geniestub/ValidationGenieService.java`
- `$HOME/code/omix/GenieStub/README-standalone.md`

Use these as contract/reference implementations for session lifecycle, detached
target control, question flow, and framework HTTP exchange usage.

## Recovery checklist

When returning to Android Agent/Genie work after interruption:

1. Read `docs/android-agent-genie-refactor.md` for the current architecture and
   recent implementation status.
2. Re-read the three markdown files inside the stub SDK zip if the framework
   contract matters for the change.
3. Check `git log --oneline -- android docs/android-agent-genie-refactor.md` to
   see the latest Android-specific changes.
4. If behavior is ambiguous, compare against the AgentStub/GenieStub reference
   implementations before changing repo code.
