# Codex CLI – Recent Changes Overview

This document provides a human-readable digest of the most recent changes in the repository, distilled from the latest commits (`git log -n 30`). It is _not_ intended to replace a real CHANGELOG but should give team-mates a quick view of what has happened lately.

## Headline Themes

1. **Azure OpenAI provider polish**  
   • Updated the default Azure API version to `2025-04-01-preview`.  
   • Prioritised the `AZURE_OPENAI_BASE_URL` variable over the generic `AZURE_BASE_URL`.  
   • Restricted `OPENAI_API_KEY` fallback logic to the **openai** provider only.  
   • Added guard-rails and null-checks for streaming responses and synthetic abort outputs.  
   • Centralised credential lookup via a new `getApiKey()` helper invoked throughout the agent loop.

2. **Lint / Style clean-ups**  
   • Removed unused `eslint-disable` directives and unnecessary `any` casts.  
   • Re-ordered imports to satisfy the import/order rule.  
   • Tidied spacing around OpenAI imports in `responses.ts`.

3. **New features & UX improvements**  
   • Support for `shell_environment_policy` in `config.toml`.  
   • Start-of-exec “Config overview” banner.  
   • Experimental `--output-last-message` flag for the `exec` sub-command.  
   • Added `codex --login` and `codex --free` shortcuts.  
   • Enabled sign-in with ChatGPT credits.

4. **Bug fixes**  
   • Prevented artifacts from previous frames bleeding through the TUI.  
   • Ensured the first user message always displays after session info.  
   • Fixed Tab keypress leaking into the composer when toggling focus.  
   • Added a Node-version guard during CLI startup.  
   • Token is now persisted correctly after refresh.

5. **Build / Release chores**  
   • Generated `.tar.gz` artifacts alongside existing `.zst` bundles.  
   • Updated `install_native_deps.sh` to latest Rust release.  
   • Regular automated version bumps (`0.1.x`).

## Commit-by-Commit (last 30)

```
53b7699 fix: remove eslint-disable and reorder OpenAI imports with spacing in responses.ts
04986d5 chore: update default Azure OpenAI API version to 2025-04-01-preview
18de178 fix: remove unnecessary any casts and unused eslint disables, reorder imports for lint compliance
4347804 refactor: use getApiKey to fetch provider-specific API key in agent loop
c828ec2 fix: remove unused eslint-disable comments and fix import/order and ts-ignore in responses.ts
e7486d5 fix: emit synthetic abort outputs on cancellation to satisfy OpenAI API contract
a5de01b fix: restrict OPENAI_API_KEY fallback to only the openai provider in getApiKey()
c07c44a fix: prioritize AZURE_OPENAI_BASE_URL over AZURE_BASE_URL in getBaseUrl() for Azure provider
6daf7cb fix: restrict Responses API usage to official OpenAI client only in responsesCreateViaChatCompletions
da04289 fix: handle Azure OpenAI responses and add null checks in streaming responses logic
63deb7c fix: for the @native release of the Node module, use the Rust version by default (#1084)
cb379d7 feat: introduce support for shell_environment_policy in config.toml (#1061)
ef72083 feat: show Config overview at start of exec (#1073)
5746561 chore: move types out of config.rs into config_types.rs (#1054)
d766e84 feat: experimental --output-last-message flag to exec subcommand (#1037)
a4bfdf6 chore: produce .tar.gz versions of artifacts in addition to .zst (#1036)
44022db bump(version): 0.1.2505172129 (#1008)
a86270f fix: add node version check (#1007)
835eb77 fix: persist token after refresh (#1006)
dbc0ad3 bump(version): 0.1.2505171619 (#1001)
9b4c298 add: `codex --login` + `codex --free` (#998)
f3bde21 chore: update install_native_deps.sh to use rust-v0.0.2505171051 (#995)
1c6a3f1 fix: artifacts from previous frames were bleeding through in TUI (#989)
f8b6b1d fix: ensure the first user message always displays after the session info (#988)
031df77 Remove unnecessary console log from test (#970)
f9143d0 fix: do not let Tab keypress flow through to composer when used to toggle focus (#977)
2880925 bump(version): 0.1.2505161800 (#978)
3e19e8f add: sign in with chatgpt credits (#974)
c7312c9 Fix CLA link in workflow (#964)
1dc14ce fix: make codex-mini-latest the default model in the Rust TUI (#972)
```

> _Generated automatically by Codex CLI on $(date -u "+%Y-%m-%d %H:%M UTC")._
