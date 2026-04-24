# Slash commands

For an overview of Codex CLI slash commands, see [this documentation](https://developers.openai.com/codex/cli/slash-commands).

## Private prompt commands

Codex also loads private prompt commands from `$CODEX_HOME/commands/`. Each Markdown file defines
one slash command whose body is submitted as a normal user prompt.

For example:

```text
$CODEX_HOME/commands/orient.md -> /orient
$CODEX_HOME/commands/db/migrate.md -> /migrate
```

Subdirectories are only for organization; the Markdown filename still provides the command name.

Command files can include optional YAML frontmatter:

```md
---
description: Rebuild working context after clearing the session
argument-hint: [focus]
---

Load my current working state into this conversation.
Focus on: $ARGUMENTS
```

`description` is shown in the slash-command popup, and `argument-hint` is shown next to the command
name. `$ARGUMENTS` expands to all text after the command name; `$1`, `$2`, and later positional
placeholders expand from shell-style split arguments.

Private command prompts are sent to the model as prompt text. Codex does not pre-execute
Claude-style ``!`command` `` substitutions or apply `allowed-tools` frontmatter from these files.
