//! Tool description prompts aligned with Claude Code v2.1.7.
//!
//! Each constant contains the full multi-paragraph description used as the
//! tool's system prompt, guiding the LLM on proper tool usage.

/// Description for the Read tool.
pub const READ_DESCRIPTION: &str = "\
Reads a file from the local filesystem. You can access any file directly by using this tool.\n\
\n\
Usage:\n\
- The file_path parameter must be an absolute path, not a relative path\n\
- By default, it reads up to 2000 lines starting from the beginning of the file\n\
- You can optionally specify a line offset and limit (especially handy for long files)\n\
- Any lines longer than 2000 characters will be truncated\n\
- Results are returned using cat -n format, with line numbers starting at 1\n\
- This tool can read images (PNG, JPG, etc), PDF files, and Jupyter notebooks (.ipynb)\n\
- This tool can only read files, not directories\n\
- You can call multiple tools in a single response for parallel reads\n\
- If you read a file that exists but has empty contents you will receive a warning";

/// Description for the Glob tool.
pub const GLOB_DESCRIPTION: &str = "\
Fast file pattern matching tool that works with any codebase size.\n\
\n\
- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n\
- Returns matching file paths sorted by modification time\n\
- Use this tool when you need to find files by name patterns\n\
- When doing an open-ended search that may require multiple rounds, use the Task tool instead\n\
- You can call multiple tools in a single response for parallel searches\n\
\n\
IMPORTANT: Omit the path field to use the default directory. \
DO NOT enter \"undefined\" or \"null\" — simply omit it.";

/// Description for the Grep tool.
pub const GREP_DESCRIPTION: &str = "\
A powerful search tool built on ripgrep.\n\
\n\
Usage:\n\
- Supports full regex syntax (e.g., \"log.*Error\", \"function\\s+\\w+\")\n\
- Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\")\n\
- Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts\n\
- Use Task tool for open-ended searches requiring multiple rounds\n\
- Pattern syntax: Uses ripgrep — literal braces need escaping\n\
- Multiline matching: By default patterns match within single lines only. For cross-line patterns, use multiline: true\n\
- head_limit and offset work across all output modes";

/// Description for the Edit tool.
pub const EDIT_DESCRIPTION: &str = "\
Performs exact string replacements in files.\n\
\n\
Usage:\n\
- You must use the Read tool at least once before editing. This tool will error if you attempt an edit without reading the file.\n\
- When editing text from Read tool output, preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix.\n\
- ALWAYS prefer editing existing files. NEVER write new files unless explicitly required.\n\
- The edit will FAIL if old_string is not unique in the file. Either provide a larger string with more context to make it unique or use replace_all.\n\
- Use replace_all for replacing and renaming strings across the file.";

/// Description for the Write tool.
pub const WRITE_DESCRIPTION: &str = "\
Writes a file to the local filesystem.\n\
\n\
Usage:\n\
- This tool will overwrite the existing file if there is one at the provided path.\n\
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.\n\
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n\
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested.";

/// Description for the Bash tool.
pub const BASH_DESCRIPTION: &str = "\
Executes a given bash command with optional timeout.\n\
\n\
IMPORTANT: This tool is for terminal operations like git, npm, docker, etc. \
DO NOT use it for file operations (reading, writing, editing, searching) — use the specialized tools instead.\n\
\n\
Usage notes:\n\
- The command argument is required\n\
- Optional timeout in milliseconds (up to 600000ms / 10 minutes). Default: 120000ms (2 minutes)\n\
- Write a clear, concise description of what this command does\n\
- If output exceeds 30000 characters, it will be truncated\n\
- Use run_in_background to run long commands without blocking\n\
- Avoid using grep/cat/find/sed/awk/echo commands — use dedicated tools instead\n\
- When issuing multiple commands: use && to chain sequential commands, make parallel calls for independent commands\n\
- Always quote file paths that contain spaces with double quotes";

/// Description for the Shell tool (array format commands).
pub const SHELL_DESCRIPTION: &str = "\
Executes a command directly without a shell interpreter.\n\
\n\
Unlike Bash, this tool takes a command as an array of strings [program, arg1, arg2, ...] \
and executes it directly via exec (no shell involved). This avoids shell parsing issues \
and is safer for commands with special characters in arguments.\n\
\n\
Usage notes:\n\
- The command parameter is a required array of strings\n\
- The first element is the program, remaining elements are arguments\n\
- Optional timeout in seconds (max 600). Default: 120 seconds\n\
- Background execution is not supported — use Bash for that\n\
- Output exceeding 30000 characters will be truncated";

/// Description for the Task tool.
pub const TASK_DESCRIPTION: &str = "\
Launch a new agent to handle complex, multi-step tasks autonomously.\n\
\n\
The Task tool launches specialized agents (subprocesses) that autonomously handle complex tasks. \
Each agent type has specific capabilities and tools available to it.\n\
\n\
When NOT to use the Task tool:\n\
- If you want to read a specific file path, use the Read or Glob tool instead\n\
- If you are searching for a specific class definition, use the Glob tool instead\n\
- If you are searching for code within a specific file, use the Read tool instead\n\
\n\
Usage notes:\n\
- Always include a short description (3-5 words) summarizing what the agent will do\n\
- Launch multiple agents concurrently whenever possible\n\
- The result returned by the agent is not visible to the user — summarize it in your response\n\
- You can run agents in the background using run_in_background\n\
- Agents can be resumed using the resume parameter\n\
- Provide clear, detailed prompts so the agent can work autonomously";

/// Description for the TaskOutput tool.
pub const TASK_OUTPUT_DESCRIPTION: &str = "\
Retrieves output from a running or completed task (background shell, agent, or remote session).\n\
\n\
- Takes a task_id parameter identifying the task\n\
- Returns the task output along with status information\n\
- Use block=true (default) to wait for task completion\n\
- Use block=false for non-blocking check of current status\n\
- Works with all task types: background shells, async agents, and remote sessions";

/// Description for the TaskStop (KillShell) tool.
pub const TASK_STOP_DESCRIPTION: &str = "\
Stops a running background task by its ID.\n\
\n\
- Takes a task_id parameter identifying the task to stop\n\
- Returns a success or failure status\n\
- Use this tool when you need to terminate a long-running task";

/// Description for the TodoWrite tool.
pub const TODO_WRITE_DESCRIPTION: &str = "\
Use this tool to create a structured task list for your current coding session. \
This helps you track progress, organize complex tasks, and demonstrate thoroughness.\n\
\n\
When to use:\n\
- Complex multi-step tasks (3+ distinct steps)\n\
- Non-trivial tasks requiring careful planning\n\
- When the user provides multiple tasks\n\
- After receiving new instructions to capture requirements\n\
\n\
When NOT to use:\n\
- Single, straightforward task\n\
- Trivial task that can be completed in less than 3 steps\n\
- Purely conversational or informational tasks\n\
\n\
Task fields:\n\
- subject: Brief, actionable title in imperative form (e.g., 'Fix authentication bug')\n\
- description: Detailed description of what needs to be done\n\
- activeForm: Present continuous form shown in spinner when in_progress (e.g., 'Fixing authentication bug')\n\
\n\
IMPORTANT: Always provide activeForm when creating tasks.";

/// Description for the EnterPlanMode tool.
pub const ENTER_PLAN_MODE_DESCRIPTION: &str = "\
Use this tool proactively when you're about to start a non-trivial implementation task. \
Getting user sign-off on your approach before writing code prevents wasted effort.\n\
\n\
When to use:\n\
1. New feature implementation\n\
2. Multiple valid approaches exist\n\
3. Code modifications that affect existing behavior\n\
4. Architectural decisions required\n\
5. Multi-file changes (more than 2-3 files)\n\
6. Unclear requirements needing exploration\n\
7. User preferences matter for the approach\n\
\n\
When NOT to use:\n\
- Single-line or few-line fixes\n\
- Adding a single function with clear requirements\n\
- User gave very specific, detailed instructions\n\
- Pure research/exploration tasks\n\
\n\
In plan mode, you'll explore the codebase, design an approach, and present your plan for approval.";

/// Description for the ExitPlanMode tool.
pub const EXIT_PLAN_MODE_DESCRIPTION: &str = "\
Use this tool when you are in plan mode and have finished writing your plan to the plan file \
and are ready for user approval.\n\
\n\
How this works:\n\
- You should have already written your plan to the plan file\n\
- This tool signals that you're done planning and ready for review\n\
- The user will see the contents of your plan file when they review it\n\
\n\
IMPORTANT: Only use this tool when the task requires planning implementation steps that require writing code. \
For research tasks where you're gathering information — do NOT use this tool.\n\
\n\
Before using: Ensure your plan is complete and unambiguous. \
If you have unresolved questions, use AskUserQuestion first.\n\
Do NOT use AskUserQuestion to ask 'Is this plan okay?' — that's what THIS tool does.";

/// Description for the AskUserQuestion tool.
pub const ASK_USER_QUESTION_DESCRIPTION: &str = "\
Use this tool when you need to ask the user questions during execution. This allows you to:\n\
1. Gather user preferences or requirements\n\
2. Clarify ambiguous instructions\n\
3. Get decisions on implementation choices as you work\n\
4. Offer choices to the user about what direction to take.\n\
\n\
Usage notes:\n\
- Users will always be able to select \"Other\" to provide custom text input\n\
- Use multiSelect: true to allow multiple answers to be selected for a question\n\
- If you recommend a specific option, make that the first option in the list and add \"(Recommended)\" at the end of the label";

/// Description for the WebFetch tool.
pub const WEB_FETCH_DESCRIPTION: &str = "\
IMPORTANT: WebFetch WILL FAIL for authenticated or private URLs.\n\
\n\
- Fetches content from a specified URL and processes it\n\
- Takes a URL and a prompt as input\n\
- Fetches the URL content, converts HTML to markdown\n\
- Processes the content with the prompt\n\
- Returns the processed response\n\
\n\
Usage notes:\n\
- The URL must be a fully-formed valid URL\n\
- HTTP URLs will be automatically upgraded to HTTPS\n\
- Results may be summarized if the content is very large\n\
- Includes a 15-minute cache for faster repeated access\n\
- When a URL redirects to a different host, make a new request with the redirect URL";

/// Description for the WebSearch tool.
pub const WEB_SEARCH_DESCRIPTION: &str = "\
Allows the agent to search the web and use the results to inform responses.\n\
\n\
- Provides up-to-date information for current events and recent data\n\
- Returns search result information with links as markdown hyperlinks\n\
- Use this tool for accessing information beyond the knowledge cutoff\n\
\n\
CRITICAL REQUIREMENT: After answering the user's question, you MUST include a 'Sources:' section \
at the end of your response listing all relevant URLs from the search results.\n\
\n\
Usage notes:\n\
- Domain filtering is supported to include or block specific websites";

/// Description for the Skill tool.
pub const SKILL_DESCRIPTION: &str = "\
Execute a skill within the main conversation.\n\
\n\
When users ask to perform tasks, check if available skills can help. \
Skills provide specialized capabilities and domain knowledge.\n\
\n\
When users ask to run a 'slash command' or reference '/<something>' \
(e.g., '/commit', '/review-pr'), they are referring to a skill.\n\
\n\
How to invoke:\n\
- Use this tool with the skill name and optional arguments\n\
- Examples: skill: 'commit', skill: 'review-pr' args: '123'\n\
\n\
Important:\n\
- When a skill is relevant, invoke this tool IMMEDIATELY as your first action\n\
- NEVER just announce a skill without actually calling this tool\n\
- Do not invoke a skill that is already running";

/// Description for the ApplyPatch tool (JSON function mode).
pub const APPLY_PATCH_DESCRIPTION: &str =
    include_str!("../../../../utils/apply-patch/apply_patch_tool_instructions.md");

/// Description for the ApplyPatch tool (freeform mode for GPT-5).
pub const APPLY_PATCH_FREEFORM_DESCRIPTION: &str = "\
Use the `apply_patch` tool to edit files. This is a FREEFORM tool — output the patch directly without JSON wrapping.\n\
\n\
Your patch language is a stripped-down, file-oriented diff format:\n\
\n\
*** Begin Patch\n\
[ one or more file sections ]\n\
*** End Patch\n\
\n\
Each operation starts with one of three headers:\n\
- *** Add File: <path> — create a new file. Following lines start with +\n\
- *** Delete File: <path> — remove an existing file\n\
- *** Update File: <path> — patch an existing file in place\n\
\n\
For Update File, use @@ to introduce hunks with context. Each hunk line starts with:\n\
- ` ` (space) for context lines\n\
- `-` for lines to remove\n\
- `+` for lines to add\n\
\n\
Important:\n\
- Include 3 lines of context before and after changes\n\
- File references must be relative, never absolute\n\
- You must prefix new lines with + even when creating a new file";

/// Description for the NotebookEdit tool.
pub const NOTEBOOK_EDIT_DESCRIPTION: &str = "\
Completely replaces the contents of a specific cell in a Jupyter notebook (.ipynb file) with new source.\n\
\n\
Jupyter notebooks are interactive documents that combine code, text, and visualizations, \
commonly used for data analysis and scientific computing.\n\
\n\
Usage:\n\
- The notebook_path parameter must be an absolute path, not a relative path\n\
- The cell_id is used to identify which cell to modify; use Read to see cell IDs first\n\
- Use edit_mode=insert to add a new cell at the index specified by cell_number\n\
- Use edit_mode=delete to delete the cell at the index specified by cell_number\n\
- When inserting a new cell, specify cell_type (code or markdown)\n\
- Prefer using Read tool first to understand the notebook structure\n\
\n\
Edit modes:\n\
- replace (default): Replace the content of an existing cell\n\
- insert: Insert a new cell (cell_type required)\n\
- delete: Delete the specified cell";

/// Description for the LS tool.
pub const LS_DESCRIPTION: &str = "\
Lists files and directories in a given path with a tree-like view.\n\
Use this tool instead of Bash ls for directory listing — it provides structured output \
with .gitignore awareness.\n\
\n\
- Default depth is 1 (immediate children only). Use depth=2+ for tree views.\n\
- Returns entries sorted with directories first, then files, alphabetically within each group\n\
- Results respect .gitignore and .ignore rules (ignored files are hidden)\n\
- Directories show with trailing /, symlinks with @\n\
- Supports pagination via offset (1-indexed) and limit for large directories\n\
- Hidden files (dotfiles) are included unless excluded by ignore rules\n\
\n\
Use Glob for pattern-based file matching or Grep for content search instead.";

/// Description for the LSP tool.
pub const LSP_DESCRIPTION: &str = "\
Language Server Protocol operations for code intelligence.\n\
\n\
Provides IDE-like features: go to definition, find references, hover documentation, \
document symbols, workspace symbols, implementation, type definition, declaration, \
call hierarchy, and diagnostics.\n\
\n\
Supported Operations:\n\
- goToDefinition: Find where a symbol is defined\n\
- findReferences: Find all usages of a symbol\n\
- hover: Get documentation/type info for a symbol\n\
- documentSymbol: List all symbols in a file\n\
- workspaceSymbol: Search for symbols across the workspace\n\
- goToImplementation: Find implementations of a trait/interface\n\
- goToTypeDefinition: Find the type definition of a symbol\n\
- goToDeclaration: Find the declaration of a symbol\n\
- getCallHierarchy: Get incoming/outgoing calls for a function\n\
- getDiagnostics: Get errors and warnings for a file\n\
\n\
Query Methods (use EITHER symbol_name OR line+character, not both):\n\
- Symbol name (AI-friendly): Specify symbol_name and optionally symbol_kind\n\
- Position (fallback): Specify line and character (both 0-indexed)\n\
\n\
Symbol Kinds: function, fn, method, class, struct, interface, trait, enum, \
variable, var, let, constant, const, property, prop, field, module, mod, type\n\
\n\
Usage notes:\n\
- This tool requires LSP servers to be installed and configured\n\
- Results include file paths and line numbers for easy navigation\n\
- For call hierarchy, use direction 'incoming' or 'outgoing'";
