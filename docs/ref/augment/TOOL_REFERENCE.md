# Augment 工具系统完整参考

## 文档信息
- **分析时间**: 2025-12-04
- **文档版本**: v2.0
- **源文件**: `chunks.64.mjs`, `chunks.76.mjs`, `chunks.77.mjs`, `chunks.78.mjs`, `chunks.82.mjs`
- **分析范围**: 所有 23 个核心工具的完整定义、描述、实现原理
- **分析状态**: ✅ 完整分析完成

---

## 工具系统架构

```
ToolHost (工具主机)
  ├── SidecarToolHost (主要工具集) - 23 个核心工具
  │   ├── FileTools (文件操作工具 - 4个)
  │   │   ├── view (E7) - 查看文件/目录
  │   │   ├── save-file (PF) - 创建新文件
  │   │   ├── str-replace-editor (ZF) - 精确编辑
  │   │   └── remove-files (MF) - 安全删除
  │   │
  │   ├── SearchTools (搜索工具 - 2个)
  │   │   ├── codebase-retrieval (VF) - AI 代码检索
  │   │   └── grep-search (AW) - Ripgrep 正则搜索
  │   │
  │   ├── EditTools (批量编辑工具 - 1个)
  │   │   └── apply_patch (zF) - V4A diff 格式编辑
  │   │
  │   ├── ProcessTools (进程管理工具 - 6个)
  │   │   ├── launch-process (Rde) - 启动进程
  │   │   ├── kill-process (Tde) - 终止进程
  │   │   ├── read-process (Hde) - 读取输出
  │   │   ├── write-process (Ide) - 写入输入
  │   │   ├── list-processes (wde) - 列出进程
  │   │   └── setup-script (Nde) - Docker 脚本测试
  │   │
  │   ├── TaskTools (任务管理工具 - 4个)
  │   │   ├── view_tasklist (xZ) - 查看任务列表
  │   │   ├── update_tasks (yZ) - 更新任务
  │   │   ├── add_tasks (RZ) - 添加任务
  │   │   └── reorganize_tasklist (CZ) - 重组任务
  │   │
  │   ├── AdvancedTools (高级功能工具 - 3个)
  │   │   ├── sub-agent (gB) - 子 Agent 委派
  │   │   ├── remember (lie) - 长期记忆管理
  │   │   └── web-fetch (pW) - 网页抓取转换
  │   │
  │   └── ContentTools (内容处理工具 - 3个)
  │       ├── view-range-untruncated (TZ) - 查看截断内容
  │       ├── search-untruncated (HZ) - 搜索截断内容
  │       └── render-mermaid (IZ) - 渲染图表
  │
  └── MCPHost (MCP 工具集)
      ├── StripeTools
      ├── SentryTools
      ├── VercelTools
      └── ...
```

---

## 1. 文件操作工具

### 1.1 view (查看文件/目录)

**工具名**: `view`
**类名**: E7
**文件位置**: chunks.76.mjs:1620
**安全级别**: Safe

**Tool Description**:
```
Custom tool for viewing files and directories and searching within files with regex query
* `path` is a file or directory path relative to the workspace root
* For files: displays the result of applying `cat -n` to the file
* For directories: lists files and subdirectories up to 2 levels deep
* If the output is long, it will be truncated and marked with `<response clipped>`

Regex search (for files only):
* Use `search_query_regex` to search for patterns in the file using regular expressions
* Use `case_sensitive` parameter to control case sensitivity (default: false)
* When using regex search, only matching lines and their context will be shown
* Use `context_lines_before` and `context_lines_after` to control how many lines of context to show (default: 5)
* Non-matching sections between matches are replaced with `...`
* If `view_range` is also specified, the search is limited to that range

Notes for using the tool:
* Strongly prefer to use `search_query_regex` instead of `view_range` when looking for a specific symbol in the file.
* Use the `view_range` parameter to specify a range of lines to view, e.g. [501, 1000] will show lines from 501 to 1000
* Indices are 1-based and inclusive
* Setting `[start_line, -1]` shows all lines from `start_line` to the end of the file
```

**Input Schema**:
```typescript
{
  type?: "file" | "directory",  // Default: "file"
  path: string,                  // Full path relative to workspace root
  view_range?: [number, number], // [start_line, end_line] (1-based, inclusive)
  search_query_regex?: string,   // Regex pattern to search
  case_sensitive?: boolean,      // Default: false
  context_lines_before?: number, // Default: 5
  context_lines_after?: number   // Default: 5
}
```

**实现原理**:
View 工具是一个多功能查看器，支持文件和目录查看，以及正则表达式搜索。对于文件，它可以显示完整内容、指定行范围或正则匹配结果。对于目录，它递归列出最多 2 层深度的文件和子目录。工具包含路径自动纠正功能，当文件不存在时会尝试查找相似路径。

**配置标志**:
- agentViewToolParams: 配置目录查看的最大深度和条目数
- view_dir_max_entries_per_depth: 每层最大条目数
- view_dir_max_depth: 最大深度

**使用场景**:
查看文件内容、浏览目录结构、在文件中搜索特定模式。优先使用 regex 搜索而非 view_range。

---

### 1.2 save-file (创建新文件)

**工具名**: `save-file`
**类名**: PF
**文件位置**: chunks.76.mjs:1405
**安全级别**: Safe

**Tool Description**:
```
Save a new file. Use this tool to write new files with the attached content. It CANNOT modify existing files. Do NOT use this tool to edit an existing file by overwriting it entirely. Use the str-replace-editor tool to edit existing files instead.
```

**Input Schema**:
```typescript
{
  path: string,              // Path of the file to save
  file_content: string,      // Content of the file
  add_last_line_newline?: boolean  // Whether to add newline at end (default: true)
}
```

**实现原理**:
Save-file 工具专门用于创建新文件，不能修改已存在的文件。它首先检查文件是否已存在，如果存在则返回错误。工具使用 checkpoint 机制记录文件创建，支持可选的换行符添加。对于空文件，工具会特别标注。

**配置标志**:
- agentSaveFileToolInstructionsReminder: 启用后会提醒限制文件内容行数
- maxLines: 150 (建议的最大行数)

**使用场景**:
创建新文件。如果文件已存在，必须使用 str-replace-editor 工具编辑。

---

### 1.3 remove-files (安全删除文件)

**工具名**: `remove-files`
**类名**: MF
**文件位置**: chunks.76.mjs:1128
**安全级别**: Safe

**Tool Description**:
```
Remove files. ONLY use this tool to delete files in the user's workspace. This is the only safe tool to delete files in a way that the user can undo the change. Do NOT use the shell or launch-process tools to remove files.
```

**Input Schema**:
```typescript
{
  file_paths: string[]  // Array of file paths to remove
}
```

**实现原理**:
Remove-files 工具通过 checkpoint 机制安全地删除文件，确保用户可以撤销更改。对于每个文件，工具首先读取文件内容，创建 checkpoint 记录原始状态，然后执行删除。这个过程使用 `_checkpointManager` 来管理版本历史。

**配置标志**:
- enableHierarchicalRules: 启用后会在输出中附加规则信息

**使用场景**:
安全删除工作区文件，保留撤销能力。绝不使用 shell 命令删除文件。

---

## 2. 代码理解工具

### 2.1 codebase-retrieval (AI 代码检索)

**工具名**: `codebase-retrieval`
**类名**: VF
**文件位置**: chunks.76.mjs:1092
**安全级别**: Safe
**版本**: 2

**Tool Description**:
```
This tool is Augment's context engine, the world's best codebase context engine. It:
1. Takes in a natural language description of the code you are looking for;
2. Uses a proprietary retrieval/embedding model suite that produces the highest-quality recall of relevant code snippets from across the codebase;
3. Maintains a real-time index of the codebase, so the results are always up-to-date and reflects the current state of the codebase;
4. Can retrieve across different programming languages;
5. Only reflects the current state of the codebase on the disk, and has no information on version control or code history.
```

**Input Schema**:
```typescript
{
  information_request: string  // Natural language description of the code you need
}
```

**实现原理**:
Codebase-retrieval 是 Augment 的核心上下文引擎，利用专有的检索和嵌入模型套件提供高质量的代码片段召回。它维护代码库的实时索引，支持跨编程语言检索，并始终反映磁盘上的当前状态。该工具调用 `agentCodebaseRetrieval` API，通过自然语言查询检索相关代码。

**使用场景**:
当需要理解代码库结构、查找特定功能实现、定位相关代码片段时使用。例如："找到所有处理用户认证的代码"、"显示数据库连接逻辑"。

---

### 2.2 grep-search (Ripgrep 搜索)

**工具名**: `grep-search`
**类名**: AW
**文件位置**: chunks.78.mjs:217
**安全级别**: Safe

**Tool Description**:
```
Runs a fast, exact regex search over text files using the ripgrep engine. Useful for finding exact text matches or patterns.

Use the following regex syntax for `query`:
[Regex syntax guide - only core features common to JavaScript and Rust]

# File filter behavior
- Only files matching `files_include_glob_pattern` would be searched
- Files matching `files_exclude_glob_pattern` would be skipped even if they match `files_include_glob_pattern`
- `files_include_glob_pattern` and `files_exclude_glob_pattern` should be standard glob patterns. Pipe character '|' is not supported.
```

**Input Schema**:
```typescript
{
  directory_absolute_path: string,
  query: string,
  case_sensitive?: boolean,          // Default: false
  files_include_glob_pattern?: string,
  files_exclude_glob_pattern?: string,
  context_lines_before?: number,     // Default: 5
  context_lines_after?: number,      // Default: 5
  disable_ignore_files?: boolean     // Default: false
}
```

**实现原理**:
Grep-search 工具使用 ripgrep 引擎进行快速的正则表达式搜索。它支持文件 glob 过滤、大小写敏感控制、上下文行显示、以及忽略文件逻辑（如 .gitignore）。工具验证输入参数，构造 ripgrep 命令，执行搜索并格式化结果。

**性能限制**:
- 超时: 10 秒
- 输出: 5000 字符

**配置标志**:
- grepSearchToolNumContextLines: 默认上下文行数（默认 5）
- grepSearchToolTimelimitSec: 超时时间
- grepSearchToolOutputCharsLimit: 输出限制

**使用场景**:
在大型代码库中快速搜索文本模式，支持正则表达式和文件过滤。

---

## 3. 代码编辑工具

### 3.1 str-replace-editor (精确编辑器)

**工具名**: `str-replace-editor`
**类名**: ZF
**文件位置**: chunks.77.mjs:180
**安全级别**: Safe

**功能**: 使用精确字符串匹配进行代码替换和插入

#### 支持的 Schema 类型

**A. Flat Schema (扁平 Schema)**

**文件位置**: `chunks.77.mjs:3-97`

```typescript
interface FlatSchemaInput {
    command: "str_replace" | "insert";
    path: string;
    instruction_reminder: string;  // 必须是固定字符串

    // str_replace 参数（可多组）
    old_str_1?: string;
    new_str_1?: string;
    old_str_start_line_number_1?: number;
    old_str_end_line_number_1?: number;

    old_str_2?: string;
    new_str_2?: string;
    old_str_start_line_number_2?: number;
    old_str_end_line_number_2?: number;

    // insert 参数（可多组）
    insert_line_1?: number;
    // new_str_1 复用

    insert_line_2?: number;
    new_str_2?: string;
}
```

**B. Nested Schema (嵌套 Schema)**

**文件位置**: `chunks.77.mjs:99-172`

```typescript
interface NestedSchemaInput {
    command: "str_replace" | "insert";
    path: string;

    // str_replace 使用数组
    str_replace_entries?: Array<{
        old_str: string;
        new_str: string;
        old_str_start_line_number: number;
        old_str_end_line_number: number;
    }>;

    // insert 使用数组
    insert_line_entries?: Array<{
        insert_line: number;
        new_str: string;
    }>;
}
```

#### Tool Description (工具描述)

**Flat Schema 描述**:
```
Tool for editing existing files.
* `path` is a file path relative to the workspace root
* `insert` and `str_replace` commands output a snippet of the edited section
  for each entry.

Notes for using the `str_replace` command:
* The `old_str_1` parameter should match EXACTLY one or more consecutive lines
  from the original file. Be mindful of whitespace!
* Empty `old_str_1` is allowed only when the file is empty or contains only
  whitespaces
* It is important to specify `old_str_start_line_number_1` and
  `old_str_end_line_number_1` to disambiguate between multiple occurrences
* Make sure line number ranges do not overlap
* The `new_str_1` parameter should contain the edited lines. Can be empty to
  delete content

Notes for using the `insert` command:
* The `insert_line_1` parameter specifies the line number AFTER which to insert
* To insert at the very beginning of the file, use `insert_line_1: 0`

IMPORTANT:
* This is the only tool you should use for editing files.
* If it fails try your best to fix inputs and retry.
* DO NOT fall back to removing the whole file and recreating it from scratch.
* DO NOT use sed or any other command line tools for editing files.
* Try to fit as many edits in one tool call as possible
* Use the view tool to read files before editing them.
* DO NOT use this tool to create new files. Use `save-file` tool instead.
```

**关键约束**:
1. ✅ **使用此工具编辑文件**
2. ❌ **不要删除整个文件重新创建**
3. ❌ **不要使用 sed 等命令行工具**
4. ✅ **尽可能在一次调用中完成多个编辑**
5. ✅ **编辑前先用 view 工具查看**

#### 高级特性

**A. 模糊匹配 (Fuzzy Matching)**

**文件位置**: `chunks.77.mjs:349-366`

```typescript
tryFuzzyMatching(
    fileContent: string,
    oldStr: string,
    newStr: string,
    startLine?: number,
    endLine?: number
)
```

**配置标志**:
- `agentEditToolEnableFuzzyMatching`
- `agentEditToolFuzzyMatchMaxDiff`
- `agentEditToolFuzzyMatchMaxDiffRatio`
- `agentEditToolFuzzyMatchMinAllMatchStreakBetweenDiffs`

**B. Tab 缩进修复**

**文件位置**: `chunks.77.mjs:329-348`

```typescript
tryTabIndentFix(
    fileContent: string,
    oldStr: string,
    newStr: string
)
```

**功能**: 自动修复 Tab 和 Space 混用导致的匹配失败

**C. 行号容错**

**配置**: `lineNumberErrorTolerance` (默认 20%)

**功能**: 允许行号有一定偏差，避免因文件变化导致的失败

**D. 自动格式化等待**

**配置**: `waitForAutoFormatMs` (默认 1000ms)

**功能**: 编辑后等待 IDE 自动格式化完成

**E. Checkpoint 管理**

**文件位置**: `chunks.77.mjs:209-225`

```typescript
async createCheckpoint(
    filepath,
    oldContent,
    newContent,
    chatHistory,
    requestId
)
```

**功能**: 每次编辑后创建检查点，支持撤销

---

### 3.2 apply_patch (V4A Diff 编辑器)

**工具名**: `apply_patch`
**类名**: zF
**文件位置**: chunks.77.mjs:911
**安全级别**: Safe

**Tool Description**:
```
This is a custom utility that makes it more convenient to add, remove, move, or edit code files. `apply_patch` effectively allows you to execute a diff/patch against a file, but the format of the diff specification is unique to this task.

*** Begin Patch
[YOUR_PATCH]
*** End Patch

Where [YOUR_PATCH] is the actual content of your patch, specified in the following V4A diff format.

*** [ACTION] File: [path/to/file] -> ACTION can be one of Add, Update, or Delete.
For each snippet of code that needs to be changed, repeat the following:
[context_before] -> See below for further instructions on context.
- [old_code] -> Precede the old code with a minus sign.
+ [new_code] -> Precede the new, replacement code with a plus sign.
[context_after] -> See below for further instructions on context.

For instructions on [context_before] and [context_after]:
- By default, show 3 lines of code immediately above and 3 lines immediately below each change.
- If 3 lines of context is insufficient to uniquely identify the snippet of code within the file, use the @@ operator to indicate the class or function to which the snippet belongs.
- If a code block is repeated so many times in a class or function such that even a single @@ statement and 3 lines of context cannot uniquely identify the snippet of code, you can use multiple `@@` statements to jump to the right context.

Note, then, that we do not use line numbers in this diff format, as the context is enough to uniquely identify code.
```

**Input Schema**:
```typescript
{
  input: string  // The complete patch text in V4A diff format
}
```

**实现原理**:
Apply_patch 实现了一个自定义的 diff/patch 格式（V4A format），不依赖行号，而是使用上下文来唯一标识代码位置。工具解析 patch 文本，识别三种操作：Add（添加文件）、Update（更新文件）、Delete（删除文件）。对于 Update 操作，它使用模糊匹配算法在文件中定位上下文，支持多级 `@@` 标记来精确定位嵌套结构。

**使用场景**:
批量文件操作，特别是需要同时添加、更新、删除多个文件时。上下文驱动的匹配比行号更稳定。

---

## 4. 进程管理工具

### 4.1 launch-process (启动进程)

**工具名**: `launch-process`
**类名**: Rde
**文件位置**: chunks.82.mjs:1232
**安全级别**: Requires Approval (based on command safety check)
**版本**: 2

**Tool Description**:
```
Launch a new process with a shell command. A process can be waiting (`wait=true`) or non-waiting (`wait=false`).

If `wait=true`, launches the process in an interactive terminal, and waits for the process to complete up to
`max_wait_seconds` seconds. If the process ends during this period, the tool call returns. If the timeout
expires, the process will continue running in the background but the tool call will return.

If `wait=false`, launches a background process in a separate terminal. This returns immediately, while the
process keeps running in the background.

Notes:
- Use `wait=true` processes when the command is expected to be short, or when you can't
proceed with your task until the process is complete.
- Use `wait=false` for processes that are expected to run in the background, such as starting a server.
- You can use this tool to interact with the user's local version control system.
```

**Input Schema**:
```typescript
{
  command: string,
  wait: boolean,
  max_wait_seconds: number,
  cwd: string  // Required: absolute path to working directory
}
```

**实现原理**:
Launch-process 工具在独立的终端中启动 shell 命令。它支持两种模式：等待模式（wait=true）会阻塞直到进程完成或超时；非等待模式（wait=false）立即返回。工具会检查命令安全性，对危险命令要求用户审批。进程启动后分配一个 terminal ID，可用于后续交互。

**使用场景**:
执行 shell 命令、启动服务器、运行测试、与版本控制交互等。

---

### 4.2 kill-process (终止进程)

**工具名**: `kill-process`
**类名**: Tde
**文件位置**: chunks.82.mjs:1327
**安全级别**: Safe

**Tool Description**:
```
Kill a process by its terminal ID.
```

**Input Schema**:
```typescript
{
  terminal_id: number
}
```

**使用场景**:
终止不再需要的后台进程或失控的进程。

---

### 4.3 read-process (读取进程输出)

**工具名**: `read-process`
**类名**: Hde
**文件位置**: chunks.82.mjs:1356
**安全级别**: Safe

**Tool Description**:
```
Read output from a terminal.

If `wait=true` and the process has not yet completed, waits for the terminal to complete up to `max_wait_seconds` seconds before returning its output.

If `wait=false` or the process has already completed, returns immediately with the current output.
```

**Input Schema**:
```typescript
{
  terminal_id: number,
  wait: boolean,
  max_wait_seconds: number
}
```

**使用场景**:
检查后台进程的输出，监控长时间运行的任务进度。

---

### 4.4 write-process (写入进程输入)

**工具名**: `write-process`
**类名**: Ide
**文件位置**: chunks.82.mjs:1401
**安全级别**: Safe

**Tool Description**:
```
Write input to a terminal.
```

**Input Schema**:
```typescript
{
  terminal_id: number,
  input_text: string
}
```

**使用场景**:
向交互式程序发送输入，如回答提示、发送命令到 REPL。

---

### 4.5 list-processes (列出进程)

**工具名**: `list-processes`
**类名**: wde
**文件位置**: chunks.82.mjs:1431
**安全级别**: Safe

**Tool Description**:
```
List all known terminals created with the launch-process tool and their states.
```

**使用场景**:
查看所有活动和已完成的进程，管理多个后台任务。

---

### 4.6 setup-script (Docker 脚本测试)

**工具名**: `setup-script`
**类名**: Nde
**文件位置**: chunks.82.mjs:1489
**安全级别**: Safe
**版本**: 2

**Tool Description**:
```
Run and validate a startup script to configure a development environment.
The setup-script tool creates a fresh sandbox environment for each invocation.
Any tools, dependencies, or configurations from previous setup-script calls are NOT preserved between calls.
Each script must include ALL necessary setup steps required for the test commands to run successfully.
```

**Input Schema**:
```typescript
{
  script_content: string,      // Bash script content
  test_commands: string[]      // Unit test commands to validate
}
```

**实现原理**:
Setup-script 工具在 Docker 沙箱中运行设置脚本并验证环境配置。它创建临时目录，复制工作区，在容器中执行脚本和测试命令，然后解析结果。每次调用都创建全新的沙箱，不保留之前的状态。

**配置标志**:
- TIMEOUT: 3600 秒（1小时）
- MAX_OUTPUT_SIZE: 10000 字符

**使用场景**:
测试和验证开发环境设置脚本，确保依赖正确安装。

---

## 5. 任务管理工具

### 5.1 view_tasklist (查看任务列表)

**工具名**: `view_tasklist`
**类名**: xZ
**文件位置**: chunks.77.mjs:1957
**安全级别**: Safe

**Tool Description**:
```
View the current task list for the conversation.
```

**实现原理**:
View_tasklist 工具显示当前对话的完整任务树结构。它从 `_taskManager` 获取根任务 UUID，然后递归展开整个任务层次结构，以 Markdown 格式呈现。任务使用特殊的状态标记：`[ ]` 未开始、`[/]` 进行中、`[-]` 已取消、`[x]` 已完成。

**使用场景**:
查看当前任务列表的完整结构和状态。

---

### 5.2 update_tasks (更新任务)

**工具名**: `update_tasks`
**类名**: yZ
**文件位置**: chunks.77.mjs:1989
**安全级别**: Safe

**Tool Description**:
```
Update one or more tasks' properties (state, name, description). Can update a single task or multiple tasks in one call. Use this on complex sequences of work to plan, track progress, and manage work.
```

**Input Schema**:
```typescript
{
  tasks: Array<{
    task_id: string,
    state?: "NOT_STARTED" | "IN_PROGRESS" | "CANCELLED" | "COMPLETE",
    name?: string,
    description?: string
  }>
}
```

**使用场景**:
批量更新任务状态、名称或描述。用于跟踪工作进度、调整任务计划。

---

### 5.3 add_tasks (添加任务)

**工具名**: `add_tasks`
**类名**: RZ
**文件位置**: chunks.77.mjs:2152
**安全级别**: Safe

**Tool Description**:
```
Add one or more new tasks to the task list. Can add a single task or multiple tasks in one call. Tasks can be added as subtasks or after specific tasks. Use this when planning complex sequences of work.
```

**Input Schema**:
```typescript
{
  tasks: Array<{
    name: string,
    description: string,
    parent_task_id?: string,
    after_task_id?: string,
    state?: "NOT_STARTED" | "IN_PROGRESS" | "CANCELLED" | "COMPLETE"
  }>
}
```

**使用场景**:
添加新任务到任务列表，可以指定为子任务或在特定任务后插入。

---

### 5.4 reorganize_tasklist (重组任务列表)

**工具名**: `reorganize_tasklist`
**类名**: CZ
**文件位置**: chunks.77.mjs:2093
**安全级别**: Safe

**Tool Description**:
```
Reorganize the task list structure for the current conversation. Use this only for major restructuring like reordering tasks, changing hierarchy. For individual task updates, use update_tasks tool.
```

**Input Schema**:
```typescript
{
  markdown: string  // Markdown representation of the complete task list
}
```

**使用场景**:
大规模重组任务列表，如重新排序、改变层次结构、批量添加/删除任务。

---

## 6. 高级功能工具

### 6.1 sub-agent (子 Agent 委派)

**工具名**: `sub-agent` (动态名称)
**类名**: gB
**文件位置**: chunks.64.mjs:798
**安全级别**: Safe

**Tool Description**:
```
Run a single synchronous sub-agent in the same workspace. Inputs: instruction (string), name (string). Returns the sub-agent's last message and minimal edit metadata. This tool only returns when the sub-agent completed its work.

**IMPORTANT: This tool can be run in parallel.** Multiple sub-agents can execute simultaneously with different names and instructions. Use parallel execution when you have multiple independent tasks that can be completed concurrently.

Available actions:
• **run** - Execute a sub-agent with the given instruction (waits for completion)
• **output** - Show the response and file changes (if any) made by a completed sub-agent
```

**Input Schema**:
```typescript
{
  action: "run" | "output",  // 'run' to execute, 'output' to retrieve results
  name: string,              // Unique name for the sub-agent (no spaces)
  instruction?: string       // Required for 'run' action only
}
```

**实现原理**:
Sub-agent 工具是一个高级委派机制，允许主 Agent 创建独立的子 Agent 来并行执行任务。每个 sub-agent 在同一工作空间中运行，并拥有独立的执行上下文。工具支持两种操作：`run` 启动新的 sub-agent 并等待其完成，`output` 获取已完成 sub-agent 的结果。

**配置标志**:
- subAgentConfig.name: 自定义 sub-agent 名称
- subAgentConfig.model: 使用的 AI 模型
- subAgentConfig.description: 功能描述

**使用场景**:
当主任务可以分解为多个独立的子任务时，使用 sub-agent 工具进行并行处理，提高效率。

---

### 6.2 remember (长期记忆管理)

**工具名**: `remember`
**类名**: lie
**文件位置**: chunks.77.mjs:1277
**安全级别**: Safe

**Tool Description**:
```
Call this tool when user asks you:
- to remember something
- to create memory/memories

Use this tool only with information that can be useful in the long-term.
Do not use this tool for temporary information.
```

**Input Schema**:
```typescript
{
  memory: string  // The concise (1 sentence) memory to remember
}
```

**实现原理**:
Remember 工具实现了一个智能记忆管理系统。它维护一个 memories 文件，使用 AI 模型将新记忆注入到现有记忆列表中，而不是简单追加。当记忆数量超过上限时，工具会自动压缩记忆，保留最近的重要信息。

**配置标志**:
- memoriesParams.injection_prompt: 记忆注入提示词
- memoriesParams.compression_prompt: 记忆压缩提示词
- memoriesParams.upper_bound_size: 记忆数量上限
- memoriesParams.num_recent_memories_to_keep: 保留最近记忆数量

**使用场景**:
存储长期有用的信息，如用户偏好、项目约定、重要决策等。不用于临时信息。

---

### 6.3 web-fetch (网页抓取转换)

**工具名**: `web-fetch`
**类名**: pW
**文件位置**: chunks.77.mjs:1114
**安全级别**: Requires Approval

**Tool Description**:
```
Fetches data from a webpage and converts it into Markdown.

1. The tool takes in a URL and returns the content of the page in Markdown format;
2. If the return is not valid Markdown, it means the tool cannot successfully parse this page.
```

**Input Schema**:
```typescript
{
  url: string  // The URL to fetch
}
```

**实现原理**:
Web-fetch 工具使用 Turndown 库将 HTML 转换为 Markdown。它包含多个自定义规则来处理代码块、行号等特殊元素。对于 HTML 页面，工具尝试使用 Turndown 转换，失败时回退到基本的 HTML-to-text 转换。工具支持内容截断，当输出超过 63KB 时会截断并存储完整内容。

**配置标志**:
- _maxOutputLength: 63 * 1024 (最大输出长度)
- _enableUntruncatedContentStorage: 启用完整内容存储

**使用场景**:
获取网页内容并转换为 Markdown 格式，用于分析文档、API 文档、技术博客等。需要用户审批。

---

## 7. 内容处理工具

### 7.1 view-range-untruncated (查看未截断内容范围)

**工具名**: `view-range-untruncated`
**安全级别**: Safe (1)
**文件位置**: `chunks.78.mjs:3-72`

**功能**: 查看被截断内容的特定行范围

**参数**:
```typescript
{
    reference_id: string;     // 截断内容的引用 ID
    start_line: number;       // 起始行号（1-based, inclusive）
    end_line: number;         // 结束行号（1-based, inclusive）
}
```

**Tool Description**:
```
View a specific range of lines from untruncated content
```

**输出示例**:
```
<!-- toolType: grep-search -->
<!-- original command: "grep -r 'function' ." -->
Showing lines 100-150 of 500 total lines

100: function processData() {
101:     // ...
...
150: }
```

### 4.2 search-untruncated (搜索未截断内容)

**工具名**: `search-untruncated`
**安全级别**: Safe (1)
**文件位置**: `chunks.78.mjs:74-134`

**功能**: 在未截断内容中搜索

**参数**:
```typescript
{
    reference_id: string;     // 截断内容的引用 ID
    search_term: string;      // 搜索关键词
    context_lines?: number;   // 上下文行数（默认 2）
}
```

**输出示例**:
```
Found 5 matches for "error" in lines 45-89 of 500 total lines

45: try {
46:     processData();
47: } catch (error) {  // ← match
48:     console.error(error);  // ← match
49: }
```

---

## 5. 可视化工具

### 5.1 render-mermaid (渲染 Mermaid 图表)

**工具名**: `render-mermaid`
**安全级别**: Safe (1)
**文件位置**: `chunks.78.mjs:155-191`

**功能**: 渲染 Mermaid 流程图、架构图等

**参数**:
```typescript
{
    diagram_definition: string;  // Mermaid 图表定义代码
    title?: string;             // 图表标题（默认 "Mermaid Diagram"）
}
```

**Tool Description**:
```
Render a Mermaid diagram from the provided definition. This tool takes Mermaid
diagram code and renders it as an interactive diagram with pan/zoom controls
and copy functionality.
```

**使用示例**:
```json
{
    "diagram_definition": "graph TD\n    A[Start] --> B[Process]\n    B --> C[End]",
    "title": "My Workflow"
}
```

**输出格式**:
```json
{
    "type": "mermaid_diagram",
    "diagram_definition": "graph TD...",
    "title": "My Workflow"
}
```

---

## 6. 任务管理工具

**文件位置**: `chunks.78.mjs:413` (引用)

**工具列表** (需要 `enableTaskList` 标志):
- `xZ`: Task tool (未详细分析)
- `CZ`: Task tool (未详细分析)
- `yZ`: Task tool (未详细分析)
- `RZ`: Task tool (未详细分析)

**启用条件**:
```javascript
if (taskManager && clientFeatureFlags.flags.enableTaskList) {
    tools.push(
        new xZ(taskManager),
        new CZ(taskManager),
        new yZ(taskManager),
        new RZ(taskManager)
    );
}
```

---

## 7. Sub-Agent 工具

### 7.1 Task (子 Agent 任务委派)

**工具名**: `gB` (需要确认实际名称)
**文件位置**: `chunks.78.mjs:414-427`

**功能**: 委派任务给 sub-agent

**启用条件**:
```javascript
if (subAgentRunner && clientFeatureFlags.flagsV2?.beachheadEnableSubAgentTool) {
    // 支持两种模式：
    // 1. 通用 sub-agent
    tools.push(new gB({ stateManager, runner: subAgentRunner }));

    // 2. 专用 sub-agent（带配置）
    for (let config of subAgentConfigs) {
        tools.push(new gB({
            stateManager,
            runner: subAgentRunner,
            subAgentConfig: config
        }));
    }
}
```

---

## 8. MCP (Model Context Protocol) 工具

### 8.1 合作伙伴 MCP 服务器

**文件位置**: `chunks.78.mjs:473-667`

| 服务 | MCP Server ID | 认证方式 | URL |
|------|--------------|---------|-----|
| **Stripe** | augment-partner-remote-mcp-stripe | OAuth | https://mcp.stripe.com |
| **Sentry** | augment-partner-remote-mcp-sentry | OAuth | https://mcp.sentry.dev/mcp |
| **Vercel** | augment-partner-remote-mcp-vercel | OAuth | https://mcp.vercel.com |
| **Render** | augment-partner-remote-mcp-render | Header | https://mcp.render.com/mcp |
| **Honeycomb** | augment-partner-remote-mcp-honeycomb | OAuth | https://mcp.honeycomb.io/mcp |
| **Postman** | augment-partner-remote-mcp-postman | Header | https://mcp.postman.com/mcp |
| **Figma** | augment-partner-remote-mcp-figma | OAuth | https://mcp.figma.com/mcp |

**工具定义结构**:
```typescript
interface MCPToolDefinition {
    definition: {
        name: string;
        description: string;
        input_schema_json: string;
        tool_safety: number;
        mcp_server_name: string;
        mcp_tool_name: string;
        original_mcp_server_name: string;
    };
    identifier: {
        hostName: string;
        toolId: string;
    };
    isConfigured: boolean;
    enabled: boolean;
    toolSafety: number;
}
```

---

## 9. 工具加载策略

### 9.1 按模式加载 (Mode-based Loading)

**文件位置**: `chunks.78.mjs:402-471`

```typescript
class SidecarToolHost {
    constructor(chatMode, clientFeatureFlags, ...) {
        const tools = [];

        if (chatMode === "REMOTE_AGENT") {
            // 远程 Agent 工具集
            tools.push(
                new pW(...),   // web-fetch
                new VF(),      // codebase-retrieval
                new MF(),      // remove-files
                new PF(),      // save-file
                new ZF(),      // str-replace-editor
                new E7(),      // view
                new IZ()       // render-mermaid
            );

            if (enableApplyPatchTool) {
                tools.push(new zF());  // Patch 应用
            }

            if (grepSearchToolEnable) {
                tools.push(new AW());  // Ripgrep 搜索
            }

            if (untruncatedContentManager) {
                tools.push(new TZ(), new HZ());  // 未截断内容工具
            }
        }
        else if (chatMode === "CLI_AGENT" || chatMode === "CLI_NONINTERACTIVE") {
            // CLI Agent 工具集
            tools.push(...);

            if (enableTaskList) {
                tools.push(new xZ(), new CZ(), new yZ(), new RZ());
            }

            if (enableSubAgentTool) {
                tools.push(new gB());
            }
        }
        else if (chatMode === "AGENT") {
            // Agent 工具集（最完整）
            tools.push(...);

            if (memoryEnabled) {
                tools.push(new lie());  // Remember 工具
            }

            if (agentEditTool === "strReplaceEditor") {
                tools.push(new ZF(), new E7());  // 编辑工具
            }
        }

        // 去重
        this.tools = removeDuplicates(tools);
    }
}
```

### 9.2 工具去重

**文件位置**: `chunks.78.mjs:429-436`

```typescript
const seenToolNames = new Set();
const uniqueTools = [];
const duplicates = [];

for (let tool of tools) {
    let toolName = tool.name.toString();
    if (seenToolNames.has(toolName)) {
        duplicates.push(toolName);
    } else {
        seenToolNames.add(toolName);
        uniqueTools.push(tool);
    }
}

if (duplicates.length > 0) {
    logger.warn(
        `Found duplicate tools that were removed: ${duplicates.join(", ")}.` +
        `Only the first occurrence of each tool was kept.`
    );
}
```

---

## 10. 工具安全级别

```typescript
enum ToolSafety {
    SAFE = 1,              // 安全操作（只读或可撤销的写操作）
    REQUIRES_APPROVAL = 2  // 需要用户批准（危险操作）
}
```

**安全工具（SAFE = 1）**:
- **文件操作**: view, save-file, remove-files, str-replace-editor
- **代码理解**: codebase-retrieval, grep-search
- **编辑**: apply_patch
- **进程管理**: kill-process, read-process, write-process, list-processes, setup-script
- **任务管理**: view_tasklist, update_tasks, add_tasks, reorganize_tasklist
- **高级功能**: sub-agent, remember
- **内容处理**: view-range-untruncated, search-untruncated, render-mermaid

**需批准工具（REQUIRES_APPROVAL = 2）**:
- **web-fetch**: 网络请求，需要用户批准
- **launch-process**: 执行 shell 命令，根据命令安全检查决定是否需要批准

**安全设计原则**:
1. 文件操作工具通过 checkpoint 机制实现撤销，因此标记为 SAFE
2. 网络请求和进程启动可能有安全风险，需要用户审批
3. launch-process 会检查命令内容，危险命令（如 rm -rf）会要求批准

---

## 11. 工具调用流程

```
┌──────────────────────────────────────────┐
│ 1. LLM 决定使用工具                       │
│    {                                      │
│      "name": "grep-search",               │
│      "arguments": {...}                   │
│    }                                      │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 2. 前端验证                               │
│    - checkToolCallSafe()                  │
│    - validateInputs()                     │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 3. 工具执行                               │
│    - tool.call(args, chatHistory, abort)  │
│    - 可能创建 checkpoint                   │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 4. 返回结果                               │
│    {                                      │
│      text: "...",                         │
│      isError: false,                      │
│      metadata: {...}                      │
│    }                                      │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 5. LLM 处理结果                           │
│    - 继续执行或返回给用户                   │
└──────────────────────────────────────────┘
```

---

## 12. 工具最佳实践（从 Tool Description 学习）

### 12.1 明确约束

```
✅ "This is the ONLY tool you should use for editing files"
✅ "DO NOT fall back to removing the whole file"
✅ "DO NOT use sed or other command line tools"
```

### 12.2 提供使用指南

```
✅ "Use the view tool to read files before editing them"
✅ "Try to fit as many edits in one tool call as possible"
```

### 12.3 参数说明详细

```
✅ "The `old_str_start_line_number_1` parameter is 1-based line number"
✅ "Both start and end line numbers are INCLUSIVE"
✅ "Be mindful of whitespace!"
```

---

## 13. 工具分类总结

### 按功能分类

| 分类 | 工具数量 | 工具列表 |
|------|---------|---------|
| **文件操作** | 4 | view, save-file, remove-files, str-replace-editor |
| **代码理解** | 2 | codebase-retrieval, grep-search |
| **批量编辑** | 1 | apply_patch |
| **进程管理** | 6 | launch-process, kill-process, read-process, write-process, list-processes, setup-script |
| **任务管理** | 4 | view_tasklist, update_tasks, add_tasks, reorganize_tasklist |
| **高级功能** | 3 | sub-agent, remember, web-fetch |
| **内容处理** | 3 | view-range-untruncated, search-untruncated, render-mermaid |
| **总计** | **23** | |

### 按模式分布

| 模式 | 可用工具类型 | 特点 |
|------|------------|------|
| **CHAT** | 基础工具（view, search） | 简单问答 |
| **AGENT** | 完整工具集 | 自主执行 |
| **REMOTE_AGENT** | 远程工具 | 无需用户交互 |
| **CLI_AGENT** | CLI 工具 + Task | 命令行模式 |
| **CLI_NONINTERACTIVE** | 基础 CLI 工具 | 脚本执行 |

### 工具能力矩阵

| 能力 | 工具 | 并行支持 | Checkpoint | 版本 |
|------|------|---------|-----------|------|
| **文件查看** | view | ❌ | ❌ | - |
| **文件创建** | save-file | ❌ | ✅ | - |
| **文件删除** | remove-files | ❌ | ✅ | - |
| **精确编辑** | str-replace-editor | ❌ | ✅ | - |
| **批量编辑** | apply_patch | ❌ | ✅ | - |
| **AI 检索** | codebase-retrieval | ❌ | ❌ | v2 |
| **正则搜索** | grep-search | ❌ | ❌ | - |
| **进程启动** | launch-process | ✅ | ❌ | v2 |
| **子任务委派** | sub-agent | ✅ (明确支持) | ❌ | - |
| **记忆管理** | remember | ❌ | ❌ | - |
| **脚本测试** | setup-script | ❌ | ❌ | v2 |

---

## 14. 关键技术发现

### 14.1 Checkpoint 机制

所有文件修改操作（save-file, remove-files, str-replace-editor, apply_patch）都使用 checkpoint 机制：
- 每次修改前保存原始状态
- 支持撤销操作
- 通过 `_checkpointManager` 管理版本历史
- 这使得"危险"的文件操作实际上是安全的

### 14.2 并行执行能力

**sub-agent 工具明确支持并行**：
- 多个 sub-agent 可以同时运行
- 每个 sub-agent 有独立的名称和执行上下文
- 通过 `subAgentId` 索引存储的结果
- 主 Agent 可以启动多个 sub-agent 并行处理独立任务

**launch-process 也支持并行**：
- 可以启动多个后台进程（wait=false）
- 通过 terminal_id 管理多个进程
- 支持同时运行多个服务器、测试等

### 14.3 智能特性

**str-replace-editor 的智能匹配**：
1. 逐字匹配（verbatim matching）
2. Tab 缩进自动修复
3. 模糊匹配（fuzzy matching）
4. 行号容错（20% 默认容错率）
5. IDE 格式化检测和等待

**remember 工具的智能压缩**：
1. 使用 AI 模型注入新记忆（而非简单追加）
2. 自动压缩超过上限的记忆
3. Ring buffer 跟踪最近记忆
4. 压缩时优先保留最近的重要信息

**codebase-retrieval 的实时索引**：
1. 维护代码库的实时索引
2. 专有的检索/嵌入模型套件
3. 跨编程语言检索
4. 始终反映磁盘当前状态

### 14.4 V4A Diff 格式

**apply_patch 的创新**：
- 不依赖行号，使用上下文匹配
- 支持多级 `@@` 标记定位嵌套结构
- 比传统 diff 更稳定（文件变化时不易失效）
- 支持 Add/Update/Delete 三种操作

### 14.5 安全设计

**多层安全机制**：
1. **工具安全级别**: SAFE vs REQUIRES_APPROVAL
2. **命令安全检查**: launch-process 检查危险命令
3. **Checkpoint 撤销**: 文件操作可撤销
4. **沙箱隔离**: setup-script 在 Docker 中运行
5. **用户审批**: web-fetch 等需要明确批准

---

**文档创建时间**: 2025-12-04
**文档版本**: v2.0
**分析状态**: ✅ 所有 23 个核心工具完整分析完成
