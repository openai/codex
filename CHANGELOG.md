# Changelog

You can install any of these versions: `npm install -g codex@version`

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

### 🐛 Bug Fixes

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

### 🐛 Bug Fixes

- Update context left display logic in TerminalChatInput component (#307)
- Improper spawn of sh on Windows Powershell (#318)
- `/bug` report command, thinking indicator (#381)
- Include pnpm lock file (#377)

## `0.1.2504172351`

### 🚀 Features

- Add Nix flake for reproducible development environments (#225)

### 🐛 Bug Fixes

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

### 🐛 Bug Fixes

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
