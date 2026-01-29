# Tool Usage Policy

- Use specialized tools instead of shell commands when available
- Use Read for reading files (not cat/head/tail)
- Use Edit for modifying files (not sed/awk)
- Use Write for creating files (not echo/heredoc)
- Use Grep for searching file contents (not grep/rg)
- Use Glob for finding files by pattern (not find/ls)
- Reserve Bash for actual system commands and terminal operations
- Call multiple independent tools in parallel when possible
- Never use placeholder values in tool parameters
- Read files before editing them
