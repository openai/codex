pub(crate) const AUTO_SCOPE_SYSTEM_PROMPT: &str = "You are an application security engineer helping select the minimal set of directories that should be examined for a security review. Only respond with JSON lines that follow the requested schema.";
pub(crate) const AUTO_SCOPE_PROMPT_TEMPLATE: &str = r#"
You are assisting with an application security review. Identify the minimal set of directories that should be in scope.

# Repository overview
{repo_overview}

# Request
<intent>{user_query}</intent>

# Request keywords
{keywords}

# Conversation history
{conversation}

# How to investigate
Use the available tools to gather evidence before selecting directories:
- list_dir: inspect directory contents when you need structure/context.
- read_file: inspect source files (read roughly {read_window} lines by default, or specify explicit line ranges).
- grep_files: search for keywords in code; use include/path/limit filters when helpful.

Issue one tool call at a time and wait for the result before deciding. When you have enough information, respond only with JSON Lines as described below.

# Selection rules
- Prefer code that serves production traffic, handles external input, or configures deployed infrastructure.
- Return directories (not files). Use the highest level that contains the relevant implementation; avoid returning both a parent and its child.
- Skip tests, docs, vendored dependencies, caches, build artefacts, editor configuration, or directories that do not exist.
- Limit to the most relevant 3–8 directories when possible.
- Before including a directory, confirm it clearly relates to <intent>{user_query}</intent>; use grep_files or read_file to look for matching terminology (README, module names, config files) when uncertain.

# Output format
Return JSON Lines: each line must be a single JSON object with keys {"path", "include", "reason"}. Omit fences and additional commentary. If unsure, set include=false and explain in reason. Output `ALL` alone on one line to include the entire repository.
"#;
pub(crate) const AUTO_SCOPE_JSON_GUARD: &str =
    "Respond only with JSON Lines as described. Do not include markdown fences, prose, or lists.";
pub(crate) const AUTO_SCOPE_KEYWORD_SYSTEM_PROMPT: &str = "You expand security review prompts into concise code search keywords. Respond only with JSON Lines.";
pub(crate) const AUTO_SCOPE_KEYWORD_PROMPT_TEMPLATE: &str = r#"
Determine the most relevant search keywords for the repository request below. Produce at most {max_keywords} keywords.

Request:
{user_query}

Guidelines:
- Prefer feature, component, service, or technology names that are likely to appear in directory names.
- Keep each keyword to 1–3 words; follow repository naming conventions (snake_case, kebab-case) when obvious.
- Skip generic words like "security", "review", "code", "bug", or "analysis".
- If nothing applies, return a single JSON object {{"keyword": "{fallback_keyword}"}} that restates the subject clearly.

Output format: JSON Lines, each {{"keyword": "<term>"}}. Do not add commentary or fences.
"#;
