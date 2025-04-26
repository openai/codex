# Changelog

You can install any of these versions: `npm install -g codex@version`

## `0.1.2504251709`

### 🚀 Features

- Add openai model info configuration (#551)
- Added provider to run quiet mode function (#571)
- Create parent directories when creating new files (#552)
- Print bug report URL in terminal instead of opening browser (#510) (#528)
- Add support for custom provider configuration in the user config (#537)
- Add support for OpenAI-Organization and OpenAI-Project headers (#626)
- Add specific instructions for creating API keys in error msg (#581)
- Enhance toCodePoints to prevent potential unicode 14 errors (#615)
- More native keyboard navigation in multiline editor (#655)
- Display error on selection of invalid model (#594)

### 🪲 Bug Fixes

- Model selection (#643)
- Nits in apply patch (#640)
- Input keyboard shortcuts (#676)
- `apply_patch` unicode characters (#625)
- Don't clear turn input before retries (#611)
- More loosely match context for apply_patch (#610)
- Update bug report template - there is no --revision flag (#614)
- Remove outdated copy of text input and external editor feature (#670)
- Remove unreachable "disableResponseStorage" logic flow introduced in #543 (#573)
- Non-openai mode - fix for gemini content: null, fix 429 to throw before stream (#563)
- Only allow going up in history when not already in history if input is empty (#654)
- Do not grant "node" user sudo access when using run_in_container.sh (#627)
- Update scripts/build_container.sh to use pnpm instead of npm (#631)
- Update lint-staged config to use pnpm --filter (#582)
- Non-openai mode - don't default temp and top_p (#572)
- Fix error catching when checking for updates (#597)
- Close stdin when running an exec tool call (#636)

## `0.1.2504221401`

### 🚀 Features

- Show actionable errors when api keys are missing (#523)
- Add CLI `--version` flag (#492)

### 🪲 Bug Fixes

- Agent loop for ZDR (`disableResponseStorage`) (#543)
- Fix relative `workdir` check for `apply_patch` (#556)
- Minimal mid-stream #429 retry loop using existing back-off (#506)
- Inconsistent usage of base URL and API key (#507)
- Remove requirement for api key for ollama (#546)
- Support `[provider]_BASE_URL` (#542)

## `0.1.2504220136`

### 🚀 Features

- Add support for ZDR orgs (#481)
- Include fractional portion of chunk that exceeds stdout/stderr limit (#497)

## `0.1.2504211509`

### 🚀 Features

- Support multiple providers via Responses-Completion transformation (#247)
- Add user-defined safe commands configuration and approval logic #380 (#386)
- Allow switching approval modes when prompted to approve an edit/command (#400)
- Add support for `/diff` command autocomplete in TerminalChatInput (#431)
- Auto-open model selector if user selects deprecated model (#427)
- Read approvalMode from config file (#298)
- `/diff` command to view git diff (#426)
- Tab completions for file paths (#279)
- Add /command autocomplete (#317)
- Allow multi-line input (#438)

### 🪲 Bug Fixes

- `full-auto` support in quiet mode (#374)
- Enable shell option for child process execution (#391)
- Configure husky and lint-staged for pnpm monorepo (#384)
- Command pipe execution by improving shell detection (#437)
- Name of the file not matching the name of the component (#354)
- Allow proper exit from new Switch approval mode dialog (#453)
- Ensure /clear resets context and exclude system messages from approximateTokenUsed count (#443)
- `/clear` now clears terminal screen and resets context left indicator (#425)
- Correct fish completion function name in CLI script (#485)
- Auto-open model-selector when model is not found (#448)
- Remove unnecessary isLoggingEnabled() checks (#420)
- Improve test reliability for `raw-exec` (#434)
- Unintended tear down of agent loop (#483)
- Remove extraneous type casts (#462)

## `0.1.2504181820`

### 🚀 Features

- Add `/bug` report command (#312)
- Notify when a newer version is available (#333)

### 🪲 Bug Fixes

- Update context left display logic in TerminalChatInput component (#307)
- Improper spawn of sh on Windows Powershell (#318)
- `/bug` report command, thinking indicator (#381)
- Include pnpm lock file (#377)

## `0.1.2504172351`

### 🚀 Features

- Add Nix flake for reproducible development environments (#225)

### 🪲 Bug Fixes

- Handle invalid commands (#304)
- Raw-exec-process-group.test improve reliability and error handling (#280)
- Canonicalize the writeable paths used in seatbelt policy (#275)

## `0.1.2504172304`

### 🚀 Features

- Add shell completion subcommand (#138)
- Add command history persistence (#152)
- Shell command explanation option (#173)
- Support bun fallback runtime for codex CLI (#282)
- Add notifications for MacOS using Applescript (#160)
- Enhance image path detection in input processing (#189)
- `--config`/`-c` flag to open global instructions in nvim (#158)
- Update position of cursor when navigating input history with arrow keys to the end of the text (#255)

### 🪲 Bug Fixes

- Correct word deletion logic for trailing spaces (Ctrl+Backspace) (#131)
- Improve Windows compatibility for CLI commands and sandbox (#261)
- Correct typos in thinking texts (transcendent & parroting) (#108)
- Add empty vite config file to prevent resolving to parent (#273)
- Update regex to better match the retry error messages (#266)
- Add missing "as" in prompt prefix in agent loop (#186)
- Allow continuing after interrupting assistant (#178)
- Standardize filename to kebab-case 🐍➡️🥙 (#302)
- Small update to bug report template (#288)
- Duplicated message on model change (#276)
- Typos in prompts and comments (#195)
- Check workdir before spawn (#221)

<!-- generated - do not edit -->
