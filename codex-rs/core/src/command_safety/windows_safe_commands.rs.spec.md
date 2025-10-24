## Overview
`core::command_safety::windows_safe_commands` enforces a read-only safelist for PowerShell commands on Windows. It parses PowerShell invocations, splits pipelines, and lets only benign cmdlets pass to auto-approval.

## Detailed Behavior
- `is_safe_command_windows`:
  - Accepts only commands starting with a PowerShell executable (`pwsh`, `powershell.exe`, etc.).
  - Uses `try_parse_powershell_command_sequence` to parse flags and scripts. It rejects encoded commands, file/script execution, unknown switches, and commands with extra trailing arguments.
- `parse_powershell_invocation` handles `-Command`, `/Command`, and `-Command:` forms, normalizing single-line scripts via `parse_powershell_script` and `split_into_commands`.
- `split_into_commands` tokenises scripts (using `shlex`), breaks them at safe separators (`|`, `||`, `&&`, `;`), and rejects tokens containing disallowed characters (`>`, `<`, `&`, `$(`) or empty segments.
- `is_safe_powershell_command` checks each command’s leading verb against an allowlist (e.g., `Get-ChildItem`, `Get-Content`, `Measure-Object`, read-only git and ripgrep commands). It bans cmdlets known to mutate state (`Set-Content`, `Remove-Item`, etc.) and prohibits redirection or the call operator.
- Helper functions rewrite nested commands to catch hidden unsafe verbs and prevent bypass via parentheses.
- Tests cover safe pipelines, git/ripgrep usage, redirection blocks, call operator rejection, and nested unsafe cmdlets.

## Broader Context
- Used by `is_safe_command` when running on Windows. This ensures shell commands are only auto-approved if they are clearly read-only, avoiding mistakes in environments where PowerShell semantics differ from Unix shells.
- The implementation emphasises conservative parsing; many benign commands may still require manual approval, but this approach minimises accidental modifications.
- Context can't yet be determined for future support of CMD or other shells; currently, anything outside PowerShell is considered unsafe.

## Technical Debt
- None explicitly, though future enhancements might extend the safe cmdlet list once thoroughly vetted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./is_safe_command.rs.spec.md
  - ../shell.rs.spec.md
