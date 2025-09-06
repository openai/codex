## Highlights:

### New Features
- Queued messages (#2637)
- Copy Paste / Drag & Drop image files (#2567)
- Add web_search tool (#2371)
- Add transcript mode (Ctrl+T) with scrolling ability (#2525)
- Edit/resume conversation (esc-esc) from previous messages (#2607)

### TUI
- Hide CoT by default; show headers in status indicator (#2316)
- Show diff output in pager (+ with hunk headers) (#2568)
- Simplify command approval UI (#2708)
- Unify Esc/Ctrl+C interrupt handling (#2661)
- Fix windows powershell paste (#2544)

### Tools and execution
- Add support for long-running shell commands `exec_command`/`write_stdin` (#2574)
- Improve apply_patch reliability (#2646)
- Cap retry counts (#2701)
- Sort MCP tools deterministically (#2611)

### Misc
- Add model_verbosity config for GPT-5 (#2108)
- Read all AGENTS.md files up to git root (#2532)
- Fix git root resolution in worktrees (#2585)
- Improve error messages & handling (#2695, #2587, #2640, #2540)


## Full list of merged PRs:

 - #2708 [feat] Simplify command approval UI
 - #2706 [chore] Tweak...

