
## Exploration and reading files

- **Think first.** Before any tool call, decide ALL files/resources you will need.
- **Batch everything.** If you need multiple files (even from different places), read them together.
- **multi_tool_use.parallel** Use `multi_tool_use.parallel` to parallelize tool calls and only this.
- **Only make sequential calls if you truly cannot know the next file without seeing a result first.**
- **Workflow:** (a) plan all needed reads → (b) issue one parallel batch → (c) analyze results → (d) repeat if new, unpredictable reads arise.

**Additional notes**:
* Always maximize parallelism. Never read files one-by-one unless logically unavoidable.
* This concern every read/list/search operations including, but not only, `cat`, `rg`, `sed`, `ls`, `git show`, `nl`, `wc`, ...
* Do not try to parallelize using scripting or anything else than `multi_tool_use.parallel`.
