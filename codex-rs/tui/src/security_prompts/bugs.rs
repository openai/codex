pub(crate) const BUGS_SYSTEM_PROMPT: &str = "You are an application security engineer reviewing a codebase.\nYou read the provided project context and code excerpts to identify concrete, exploitable security vulnerabilities.\nFor each vulnerability you find, produce a thorough, actionable write-up that a security team could ship directly to engineers.\n\nStrict requirements:\n- Only report real vulnerabilities with a plausible attacker-controlled input and a meaningful impact.\n- Quote exact file paths and GitHub-style line fragments, e.g. `src/server/auth.ts#L42-L67`.\n- Provide dataflow analysis (source, propagation, sink) where relevant.\n- Include a severity rating (high, medium, low, ignore) plus impact and likelihood reasoning.\n- Include a taxonomy line exactly as `TAXONOMY: {...}` containing JSON with keys vuln_class, cwe_ids[], owasp_categories[], vuln_tag.\n- If you cannot find a security-relevant issue, respond with exactly `no bugs found`.\n- Do not invent commits or authors if unavailable; leave fields blank instead.\n- Keep the response in markdown.";

// The body of the bug analysis user prompt that follows the repository summary.
pub(crate) const BUGS_USER_CODE_AND_TASK: &str = r#"
# Code excerpts
{code_context}

# Task
Evaluate the project for concrete, exploitable security vulnerabilities. Prefer precise, production-relevant issues to theoretical concerns.

Follow these rules:
- Read this file in full and review the provided context to understand intended behavior before judging safety.
{scope_reminder}- Start locally: prefer `READ` to open the current file and its immediate neighbors (imports, same directory/module, referenced configs) before using `GREP_FILES`. Use `GREP_FILES` only when you need to locate unknown files across the repository.
- When you reference a function, method, or class, look up its definition and usages across files: search by the identifier, then open the definition and a few call sites to verify behavior end-to-end.
- The current file is provided in full. Analyze it first; do not issue broad searches for generic or dangerous keywords (e.g., "password", "token") unless you are tracing a concrete dataflow across files.
- Use the search tools below to inspect additional in-scope files only when tracing data flows or confirming a hypothesis that clearly spans multiple files; cite the relevant variables, functions, and any validation or sanitization steps you discover.
- Trace attacker-controlled inputs through the call graph to the ultimate sink. Highlight any sanitization or missing validation along the way.
- Group variants that share the same root cause or control gap into one finding; list all affected paths/endpoints within that finding instead of emitting near-duplicates. When endpoints or code locations are unique but stem from the same issue, merge them into a single consolidated finding rather than dropping them.
- Ignore unit tests, example scripts, or tooling unless they ship to production in this repo.
- Only report real vulnerabilities that an attacker can trigger with meaningful impact. If none are found, respond with exactly `no bugs found` (no additional text).
- Emphasize findings with concrete impact to infrastructure control, user data exposure, or financial loss; spell out the impact path.
- Quote code snippets and locations using GitHub-style ranges (e.g., `src/service.rs#L10-L24`). Include git blame details when you have them: `<short-sha> <author> <YYYY-MM-DD> L<start>-L<end>`.
- Keep all output in markdown and avoid generic disclaimers.
- If you need more repository context, request it explicitly while staying within the provided scope:
  - Prefer `READ: <relative path>` to inspect specific files (start with the current file and immediate neighbors).
  - When you know multiple files or ranges to open, group them into a single parallel READ batch instead of separate calls.
  - Use `SEARCH: literal:<identifier>` or `SEARCH: regex:<pattern>` to locate definitions and call sites across files; then `READ` the most relevant results to confirm the dataflow.
  - Use `GREP_FILES: {"pattern":"needle","include":"*.rs","path":"subdir","limit":200}` to discover candidate locations across the repository; prefer meaningful identifiers over generic terms.

# Output format
For each vulnerability, emit a markdown block:

### <short title>
- **File & Lines:** `<relative path>#Lstart-Lend`
- **Severity:** <high|medium|low|ignore>
- **Impact:** <concise impact analysis>
- **Likelihood:** <likelihood analysis>
- **Description:** Detailed narrative with annotated code references explaining the bug.
- **Snippet:** Fenced code block (specify language) showing only the relevant lines with inline comments or numbered markers that you reference in the description.
- **Dataflow:** Describe sources, propagation, sanitization, and sinks using relative paths and `L<start>-L<end>` ranges.
    - **PoC:** Provide two variants when possible:
      - Minimal standalone snippet or test file that runs in isolation (no full project setup) to validate the specific flaw. Run it locally first to ensure the syntax is correct and that it executes successfully. You do not need a full exploit chain—focus on the precise issue (e.g., missing validation, comparison/logic error, injection behavior) and mock other components as needed. You may use the same dependencies referenced in the reviewed code.
      - Attacker-style reproduction steps or payload against the exposed interface (HTTP request, CLI invocation, message payload, etc.).
      If only one is feasible, provide that. Use fenced code blocks for code where appropriate. If the minimal PoC is lengthy, include it as file contents and specify that it should be saved under `bug-<index>-poc/` (where `<index>` is this finding’s 1-based order), with clear filenames.
- **Recommendation:** Actionable remediation guidance.
- **Verification Type:** JSON array subset of ["network_api", "crash_poc", "web_browser"].
- TAXONOMY: {{"vuln_class": "...", "cwe_ids": [...], "owasp_categories": [...], "vuln_tag": "..."}}

Ensure severity selections are justified by the described impact and likelihood."#;
