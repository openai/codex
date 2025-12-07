#![allow(dead_code)]

// Centralized prompt strings for the security review feature.

// Auto-scope prompts
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

# Available tools
- SEARCH: respond with `SEARCH: literal:<term>` or `SEARCH: regex:<pattern>` to run ripgrep over the repository root (returns colored matches with line numbers).
- GREP_FILES: respond with `GREP_FILES: {"pattern":"needle","include":"*.rs","path":"subdir","limit":200}` to list files whose contents match. Fields:
  - pattern: regex string (required)
  - include: optional glob filter (ripgrep --glob)
  - path: optional directory/file to search (defaults to repo root)
  - limit: optional max paths to return (default 100, max 2000)
- READ: respond with `READ: <relative path>#L<start>-L<end>` to inspect source code (omit the range to read roughly {read_window} lines starting at the top of the file).

Issue at most one tool command per message and wait for the tool output before continuing. When you have gathered enough information, respond only with JSON Lines as described below.

# Selection rules
- Prefer code that serves production traffic, handles external input, or configures deployed infrastructure.
- Return directories (not files). Use the highest level that contains the relevant implementation; avoid returning both a parent and its child.
- Skip tests, docs, vendored dependencies, caches, build artefacts, editor configuration, or directories that do not exist.
- Limit to the most relevant 3–8 directories when possible.
- Before including a directory, confirm it clearly relates to <intent>{user_query}</intent>; use SEARCH or READ to look for matching terminology (README, module names, config files) when uncertain.

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

// Spec generation prompts
pub(crate) const SPEC_SYSTEM_PROMPT: &str = "You are an application security engineer documenting how a project is built. Produce an architecture specification that focuses on components, flows, and controls. Stay within the provided code locations, ground claims in concrete evidence, and keep the output in markdown.";
pub(crate) const SPEC_COMBINE_SYSTEM_PROMPT: &str = "You are consolidating multiple specification drafts into a single, cohesive project specification. Merge overlapping content, keep terminology consistent, and follow the supplied template. Preserve every security-relevant detail; when in doubt, include rather than summarize away content.";
pub(crate) const SPEC_PROMPT_TEMPLATE: &str = "You have access to the source code inside the following locations:\n{project_locations}\nRepository root (use absolute paths in tool calls): {repo_root}\n\nFocus on {target_label}.\nGenerate a security-focused project specification. Before drafting, identify whether the target behaves like a backend service, desktop or mobile client, SDK/library, firmware, CLI, or mixed topology. Adapt the structure and diagrams to match that shape:\n- For services, emphasise request routing, data persistence, and external integrations.\n- For SDKs/libraries, highlight module responsibilities, host-application touchpoints, and API usage sequences.\n- For clients or firmware, show local process boundaries, platform APIs, and sync/telemetry flows.\n\nCapture the concrete flows, component responsibilities, runtime/distribution model, and exposed surfaces so the final markdown can include fit-for-purpose diagrams (architecture overview, component interactions, sequence diagrams) grounded in real handlers, modules, or functions. Do not diagram low-level runtime details (e.g., container startup sequences, helper scripts, or test utilities). When a section does not apply, state `- Not applicable (reason)` instead of leaving it blank.\n\nParallelize discovery when enumerating files and avoid spending time on tests, vendored dependencies, or build artefacts. When you already know a set of files or directories to open, issue a single parallel batch of READ calls instead of one-by-one requests.\n\nUse the available tools directly without restating how to call them. While reading files, collect exact handler names, structs, RPCs, public APIs, and configuration fields you will reference in the diagrams and Sources sections. Start by inspecting the current directory layout (READ a directory path to list entries) and open the key files you need to understand behaviour before drafting. SEARCH is unavailable. Make a few grounded tool calls before returning the final markdown.\n\nTemplate:\n{spec_template}\n";
pub(crate) const CONVERT_CLASSIFICATION_TO_JSON_PROMPT_TEMPLATE: &str = r#"
Read the project specification below and extract a normalized Data Classification list.

<specification>
{spec_markdown}
</specification>

# Goal
Produce newline-delimited JSON (NDJSON), one object per classified data type with keys:
- data_type (string — e.g., PII, PHI, PCI, credentials, secrets, telemetry)
- sensitivity (exactly one of: high, medium, low)
- storage_location (string)
- retention (short policy or duration)
- encryption_at_rest (string; use "unknown" if not stated)
- in_transit (string; use "unknown" if not stated)
- accessed_by (string describing services/roles/users)

# Guidance
- Prefer the specification's Data Classification section; infer from context when necessary.
- Merge duplicate data types, choosing the strictest sensitivity.
- Keep values concise and human-readable.

# Output
Emit only NDJSON lines. Each JSON object must contain exactly the keys listed above (no arrays, extra keys, or prose).
"#;

// Validation plan prompts
pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "You are an application security engineer planning minimal, safe validations for high-risk findings. Respond ONLY with JSON Lines as requested; do not include markdown or prose.";
pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Before any checks, create two test accounts if the app requires login. Prefer a short Python script that calls a signup endpoint or automates the registration form headlessly. If this is not feasible, return a `manual` instruction with a `login_url`.

Then select ONLY high-risk findings to validate. For each, choose the minimal tool and target:
- Use the Playwright MCP tool for web_browser checks (supply a reachable URL in `target`).
- Use tool "curl" for network_api checks (supply full URL in `target`).
- Use tool "python" only if a short, non-destructive PoC is essential (include inline script text in `script`).

Rules:
- Keep requests minimal and non-destructive; no state-changing actions.
- Prefer headless checks (e.g., page loads, HTTP status, presence of a marker string).
- Max 5 requests total; prioritize Critical/High severity or lowest risk_rank.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For account setup (emit at most one line): {"id_kind":"setup","action":"register|manual","login_url":"<string, optional>","tool":"python|manual","script":"<string, optional>"}
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"playwright|curl|python","target":"<string, optional>","script":"<string, optional>"}
"#;

// Account setup planning (standalone, used when needed)
pub(crate) const VALIDATION_ACCOUNTS_SYSTEM_PROMPT: &str = "You plan how to create two test accounts for a typical web app. Respond ONLY with JSON Lines; no prose.";
pub(crate) const VALIDATION_ACCOUNTS_PROMPT_TEMPLATE: &str = r#"
Goal: ensure two test accounts exist prior to validation. Prefer a short Python script that registers accounts via HTTP or a headless flow; otherwise return a manual login URL.

Constraints:
- The script must be non-destructive and idempotent.
- Print credentials to stdout as JSON: {"accounts":[{"username":"...","password":"..."},{"username":"...","password":"..."}]}.
- If you cannot identify a safe automated path, return a single JSON line: {"action":"manual","login_url":"https://..."}.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- Automated: {"action":"register","tool":"python","login_url":"<string, optional>","script":"<python script>"}
- Manual: {"action":"manual","login_url":"<string>"}
"#;
pub(crate) const MARKDOWN_OUTPUT_GUARD: &str = "\n# Output Guard (strict)\n    - Output only the final markdown content requested.\n    - Do not include goal, analysis, planning, chain-of-thought, or step lists.\n    - Do not echo prompt sections like \"Task\", \"Steps\", \"Output\", or \"Important\".\n    - Do not include any XML/angle-bracket blocks (e.g., <...> inputs) in the output.\n    - Do not wrap the entire response in code fences; use code fences only for code snippets.\n    - Do not include apologies, disclaimers, or references to being an AI model.\n";
pub(crate) const MARKDOWN_FIX_SYSTEM_PROMPT: &str = "You are a meticulous technical editor. Polish markdown formatting while preserving the original security analysis content. Focus on fixing numbering, bullet spacing, code fences, and diagram syntax without adding or removing information.";
pub(crate) const SPEC_COMBINE_PROMPT_TEMPLATE: &str = "You previously generated specification drafts for the following code locations:\n{project_locations}\n\nDraft content (each draft may include an \"API Entry Points\" section summarizing externally exposed interfaces):\n{spec_drafts}\n\nTask: merge these drafts into one comprehensive specification that describes the entire project. Remove duplication, keep terminology consistent, and ensure the final document reads as a single report that preserves API coverage. Follow the template exactly and return only markdown.\n\nNon-negotiable requirements:\n- Carry forward every concrete security-relevant fact, list, table, code block, and data classification entry from the drafts unless it is an exact duplicate.\n- When multiple drafts contribute to the same template section, include the union of their paragraphs and bullet points. If details differ, keep both and attribute them with inline labels such as `(from {location_label})` rather than dropping information.\n- Preserve API entry points verbatim (including tables) and incorporate them into the appropriate section without shortening columns.\n- Keep all identifiers (component names, queue names, environment variables, secrets, external services, metric names) exactly as written; do not rename or generalize.\n- Follow the template's structure exactly: populate every section, create the requested subsections, and include the explicit `Sources:` lines and bullet styles. Do not leave the instructional text in place or drop mandatory sections.\n- Populate the \"Relevant Source Files\" section with bullet points that reference each draft's location label and any concrete file paths mentioned in the drafts.\n- Ensure the \"Data Classification\" section exists even when the drafts were sparse; aggregate and preserve every classification detail there.\n- If multiple drafts contain tabular data (APIs, components, data classification), merge rows from all drafts and maintain duplicates when the sources disagree so the consumer can reconcile manually.\n- Do not introduce new speculation or remove nuance from mitigations, caveats, or risk descriptions provided in the drafts. Err on the side of length; the final document should be at least as detailed as the most verbose draft.\n\n# Available tools\n- READ: respond with `READ: <relative path>#Lstart-Lend` (range optional) to open code or draft files. Use paths relative to the repository root.\n- GREP_FILES: respond with `GREP_FILES: {\"pattern\": \"...\", \"include\": \"*.rs\", \"path\": \"subdir\", \"limit\": 200}` to list files whose contents match.\nEmit at most one tool command in a single message and wait for the tool output before continuing. Prefer READ for prose context; SEARCH is not available during this step.\n\nTemplate:\n{combined_template}\n";
pub(crate) const SPEC_DIR_FILTER_SYSTEM_PROMPT: &str = r#"
You triage directories for a security review specification. Only choose directories that hold core product or security-relevant code.
- Prefer application source directories (services, packages, libs) and entry points that process real traffic or data.
- Exclude tests, fixtures, scripts/tooling, examples, database migrations or seeding folders, build artifacts, vendored dependencies, generated code, or documentation-only folders. Do not select paths containing "migration", "migrations", "seed", or "seeder" unless no other application code exists.
- If a directory looks like helper utils (path util, user util, lfs util) or only contains migrations/seeding, skip it unless it is the only place core logic lives.
Respond with a newline-separated list containing only the directory paths chosen from the provided list. Respond with `ALL` if every directory should be included. Do not add quotes or extra commentary.
"#;
pub(crate) const SPEC_MARKDOWN_TEMPLATE: &str = "# Project Specification
- Location: {target_label}
- Prepared by: {model_name}
- Date: {date}
- In-scope paths:
```
{project_locations}
```

## Overview
Summarize what the project delivers, who uses it, and why it matters. Call out the most security-relevant entry points.

### Relevant Source Files
List bullet points referencing the primary source directories or files (use inline code formatting) with a short note describing what each covers. Make sure the key areas of the scope appear here.

## Overall Architecture
Describe how control and data move through the system. Tailor the narrative to the project type:
- Services: show request routing, data stores, and external integrations.
- SDKs/libraries: show host application calls into the library and any upstream services.
- Clients/firmware: show local processes, platform APIs, and sync/telemetry paths.
Include a `mermaid flowchart TD` (or `flowchart LR`) that shows only the core components and their compositions; if the system has many components, keep this diagram to the essential surfaces and push detailed visuals into the per-component sections below. Label edges with the action or payload and highlight trust boundaries (grouping nodes or adding annotations is fine). If you add more than one diagram anywhere in the spec, insert a `title <descriptive name>` line inside each mermaid block so rendered output shows distinct labels. Finish with `Sources:` pointing to the code that backs the description.

## Core Components
Open with a sentence or two describing the component landscape. When it clarifies relationships, include a compact `mermaid flowchart` that shows the major components/modules and their interactions.

Create `### <Component name>` subsections for roughly 5-8 components (map to services, crates/packages, critical subsystems, or hardware modules depending on the project). Within each subsection include bullet points covering:
- Primary responsibilities and scope (be concrete about behaviours and data handled)
- Key dependencies, integrations, or hardware/software interfaces
- Security-relevant behaviour or controls (validation, auth checks, rate limiting, sandboxing, etc.)
Include a focused `mermaid flowchart` for this component when it helps show detail that would clutter the overall architecture diagram. End every subsection with a line that starts with `Sources:` referencing the supporting directories or modules. If a component does not exist for this architecture, omit the subsection. Do not add an extra, section-level `Sources:` block after listing the components; the per-component sources are sufficient.

## Usage & Execution Flows
Document the critical workflows. For each important flow, create a `### <Flow name>` subsection that:
- Explains the trigger and actors
- Lists the main steps or states (bullets)
- Includes a `mermaid sequenceDiagram` or flowchart grounded in real functions/handlers/commands
- Ends with `Sources:` citing the relevant code
If authentication or authorization occurs, dedicate subsections to those flows; otherwise, state `- Not applicable (reason)`.

## Configuration & Policy Model
Summarize how runtime behaviour is configured (files, env vars, feature flags, build-time toggles, device settings). Mention validation routines, defaults, and how secrets or credentials enter the system. Include a small diagram or table when it helps. End with `Sources:`.

## Runtime & Deployment Model
Outline how the project is packaged and delivered (e.g., binaries, firmware images, mobile bundles, library artifacts), supported deployment modes, and operational differences that affect security. A compact flowchart or table is welcome. End with `Sources:`.

## Security Controls
### Authentication
Detail identity providers, credential flows, session handling, and token management (or state `- Not applicable (reason)`). Include libraries or helpers used and finish with `Sources:`.

### Authorization
Explain enforcement points, policy evaluation, privilege separation, and escalation safeguards (or mark as not applicable). Include `Sources:`.

### Auditing & Observability
Cover logging, metrics, traces, and alerting hooks used to monitor security-sensitive behaviour. Call out log redaction or PII handling. Include `Sources:`.

## Trust Boundaries & Message Channels
List the key trust boundaries (for example: browser ↔ service worker, host app ↔ SDK, microcontroller ↔ peripheral, client ↔ server, server ↔ external APIs). For each, note the data types crossing it, the transport/security guarantees (origin checks, auth, rate limits), and any validation or schema enforcement applied to messages. If you include more than one diagram in this section, add a brief introductory sentence and a `title <name>` line inside each mermaid block, and place each diagram in its own subsection with a one-line description so they are clearly labeled. End with `Sources:`.

## API & Interfaces
Describe externally reachable or embeddable surfaces.

### Server APIs
Provide a markdown table with the exact columns:
- endpoint path
- authN method
- authZ type
- request parameters
- example request (params, body, or method)
- code location
- parsing/validation logic (include auth/rate limiting/abuse controls if present)
If the project exposes no server APIs, write `- Not applicable (library/client-only)` (or similar) instead of a table.

### Client/SDK APIs
When the project ships a library, client SDK, CLI, mobile surface, or firmware commands, provide a table with columns:
- api name (module.func, CLI command, or message)
- module/package (or binary/component)
- summary
- parameters (omit if noisy)
- returns / side effects (omit if noisy)
- stability (public/official/internal/experimental)
- code location
If there is no callable client surface, state `- Not applicable (reason)`.

### Other Interfaces
Capture message queues, hardware interfaces, scheduled jobs, or integrations that do not fit the tables above. Use bullets, note security expectations, and include `Sources:` when added.

## Data Classification
Identify sensitive data types handled by the project and where they are stored or transmitted. Prefer markdown tables that consolidate the details (data type, sensitivity, storage location, retention, transport protections). End with `Sources:`.

## Secrets, Error Handling, and Logging
Enumerate required secrets (API keys, certificates, hardware tokens), how absence or invalid values fail, and any logging or error paths that could leak input or credentials. Mention rotation, storage, and masking practices. Include `Sources:`.

## Operational Considerations
Summarize infrastructure-as-code, build/deploy automation, background jobs or schedulers, scaling/resiliency patterns, device update mechanisms, and monitoring/alerting hooks that affect security posture. Close with `Sources:`.
";
pub(crate) const SPEC_COMBINED_MARKDOWN_TEMPLATE: &str = r#"# Project Specification
Provide a 2–3 sentence executive overview summarizing the system's purpose, primary users, and the highest-value assets or flows that matter for security.

## Relevant Source Files
List bullet points for the key files and directories covered by the drafts. Use inline code formatting for paths (for example, `src/service.rs`) and briefly note what each covers. Ensure every draft's location label appears at least once.

## Overall Architecture
Provide a concise overview of how control and data move through the system, adapted to the project type (services, SDKs/libraries, clients/firmware). Include exactly one overarching `mermaid flowchart TD` (or `flowchart LR`) diagram here that captures the end-to-end interaction (no per-component or sequence diagrams in this section). Keep this diagram to the core components and compositions; move detailed interactions into the per-component sections. Label edges with the action or payload being exchanged and call out trust boundaries inside the diagram (for example, group nodes with subgraphs or add text annotations). If the combined document includes other diagrams elsewhere, add a `title <descriptive label>` line inside each mermaid block so rendered output shows distinct titles. End with a `Sources:` line enumerating the supporting modules.

## Core Components
Create `### <Component name>` subsections for the union of the 4–8 major components across drafts (services, packages, subsystems, or hardware modules as appropriate). Within each subsection, merge bullets covering:
- Primary responsibilities and scope (be concrete about behaviours and data handled)
- Key dependencies, integrations, or interfaces
- Security-relevant behaviour or controls
Include a compact `mermaid flowchart` when it clarifies interactions, and use it to show detail that would clutter the overall architecture diagram. End every subsection with a `Sources:` line referencing the supporting directories (prefer directories over individual file paths unless only files were cited).

## Usage & Execution Flows
Merge the critical workflows from the drafts. Provide at least `### Authentication Flow` and `### Authorization Flow` when applicable; add other flow subsections as needed. For each flow, combine the steps, keep or merge mermaid diagrams (preserve multiple when they differ, adding descriptive titles), and finish with `Sources:` that cite every contributing draft.

## Configuration & Policy Model
Combine descriptions of configuration surfaces, policy definitions, validation, and dynamic behaviours. Merge diagrams or tables and end with `Sources:` listing every referenced module.

## Runtime & Deployment Model
Aggregate deployment or distribution details (binaries, libraries, mobile bundles, firmware images, service topologies). Note mode differences and security implications. Include diagrams or tables when present. Conclude with `Sources:`.

## Security Controls
### Authentication
Merge authentication-related details (identity providers, credential flows, session handling). Cite all sources.

### Authorization
Combine policy evaluation, enforcement points, and privilege separation details. Cite all sources.

### Auditing & Observability
Aggregate logging, metrics, tracing, and alerting content. Cite all sources.

## Trust Boundaries & Message Channels
Merge lists of trust boundaries and message channels. If multiple diagrams exist, include them all with clear titles. End with `Sources:`.

## API & Interfaces
Union all externally facing surfaces. Preserve and merge tables for server APIs, client/SDK APIs, and other interfaces exactly (include duplicate rows when the drafts disagree). Cite all sources.

## Data Classification
Aggregate the data classification entries from the drafts into a single table or list. Keep duplicates when values differ so the consumer can reconcile them. End with `Sources:`.

## Secrets, Error Handling, and Logging
Combine guidance on secrets management, error handling, logging, and potential leakage paths. Include every relevant source citation.

## Operational Considerations
Merge deployment, infrastructure-as-code, automation, scaling, resiliency, and monitoring/alerting details. Cite all supporting modules.

"#;

// Threat model prompts
pub(crate) const THREAT_MODEL_SYSTEM_PROMPT: &str = "You are a senior application security engineer preparing a threat model. Use the provided architecture specification and repository summary to enumerate realistic threats, prioritised by risk.";
pub(crate) const THREAT_MODEL_PROMPT_TEMPLATE: &str = "# Repository Summary\n{repository_summary}\n\n# Architecture Specification\n{combined_spec}\n\n# In-Scope Locations\n{locations}\n\n# Task\nConstruct a concise threat model for the system. Focus on meaningful attacker goals and concrete impacts.\n\n## Output Requirements\n- Start with a short paragraph summarising the most important threat themes and high-risk areas.\n- Immediately after the summary, add a short bullet list of the top abuse paths (for example, unauthenticated endpoints, cross-origin messaging gaps, or missing rate limits) before the table.\n- Follow with a markdown table named `Threat Model` with columns: `Threat ID`, `Threat source`, `Prerequisites`, `Threat action`, `Threat impact`, `Impacted assets`, `Priority`, `Recommended mitigations`.\n- Use integer IDs starting at 1. Priority must be one of high, medium, low.\n- Keep prerequisite and mitigation text succinct (single sentence each).\n- Do not include any other sections or commentary outside the summary paragraph, abuse-path bullets, and table.\n";

// Bug analysis prompts
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
- Quote code snippets and locations using GitHub-style ranges (e.g. `src/service.rs#L10-L24`). Include git blame details when you have them: `<short-sha> <author> <YYYY-MM-DD> L<start>-L<end>`.
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

// Bug rerank prompts
pub(crate) const BUG_RERANK_SYSTEM_PROMPT: &str = "You are a senior application security engineer triaging review findings. Reassess customer-facing risk using the supplied repository context and previously generated specs. Only respond with JSON Lines.";
pub(crate) const BUG_RERANK_PROMPT_TEMPLATE: &str = r#"
Repository summary (trimmed):
{repository_summary}

Spec excerpt (trimmed; pull in concrete details or note if unavailable):
{spec_excerpt}

Examples:
- External unauthenticated remote code execution on a production API ⇒ risk_score 95, severity "High", reason "unauth RCE takeover".
- Stored XSS on user dashboards that leaks session tokens ⇒ risk_score 72, severity "High", reason "persistent session theft".
- Originally escalated CSRF on an internal admin tool behind SSO ⇒ risk_score 28, severity "Low", reason "internal-only with SSO".
- Header injection in a deprecated endpoint with response sanitization ⇒ risk_score 18, severity "Informational", reason "sanitized legacy endpoint".
- Static analysis high alert that only touches dead code ⇒ risk_score 10, severity "Informational", reason "dead code path".
- High-severity SQL injection finding that uses fully parameterized queries ⇒ risk_score 20, severity "Low", reason "parameterized queries".
- SSRF flagged as critical but the target requires internal metadata access tokens ⇒ risk_score 24, severity "Low", reason "internal metadata token".
- Critical-looking command injection in an internal-only CLI guarded by SSO and audited logging ⇒ risk_score 22, severity "Low", reason "internal CLI".
- Reported secret leak found in sample dev config with rotate-on-startup hook ⇒ risk_score 12, severity "Informational", reason "sample config only".

# Available tools
- READ: respond with `READ: <relative path>#Lstart-Lend` (range optional) to inspect specific source code.
- SEARCH: respond with `SEARCH: literal:<term>` or `SEARCH: regex:<pattern>` to run ripgrep over the repository root (returns colored matches with line numbers).
- GREP_FILES: respond with `GREP_FILES: {"pattern":"needle","include":"*.rs","path":"subdir","limit":200}` to list files whose contents match, ordered by modification time.
- Issue at most one tool command per round and wait for the tool output before continuing. Reuse earlier tool outputs when possible.

Instructions:
- Output severity **only** from ["High","Medium","Low","Informational"]. Map "critical"/"p0" to "High".
- Produce `risk_score` between 0-100 (higher means greater customer impact) and use the full range for comparability.
- Review the repository summary, spec excerpt, blame metadata, and file locations before requesting anything new; reuse existing specs or context attachments when possible.
- If you still lack certainty, request concrete follow-up (e.g., repo_search, read_file, git blame) in the reason and cite the spec section you need.
- Reference concrete evidence (spec section, tool name, log line) in the reason when you confirm mitigations or reclassify a finding.
- Prefer reusing existing tool outputs and cached specs before launching new expensive calls; only request fresh tooling when the supplied artifacts truly lack the needed context.
- Prioritize issues that meaningfully threaten infrastructure control, user data, or financial integrity; make the impact explicit in the reason.
- Down-rank issues when mitigations or limited blast radius materially reduce customer risk, even if the initial triage labeled them "High". For DoS, only consider elevated risk when autoscaling or graceful recovery cannot mitigate an availability loss that causes real user/data impact.
- De-emphasize DoS findings that are just repeated calls without resource amplification (memory/CPU/locks); if no meaningful exhaustion or lockout risk exists, ignore the finding.
- Upgrade issues when exploitability or exposure was understated, or when multiple components amplify the blast radius.
- Respond with one JSON object per finding, **in the same order**, formatted exactly as:
  {{"id": <number>, "risk_score": <0-100>, "severity": "<High|Medium|Low|Informational>", "reason": "<≤12 words>"}}

Findings:
{findings}
"#;

// File triage prompts
pub(crate) const FILE_TRIAGE_SYSTEM_PROMPT: &str = "You are an application security engineer triaging source files to decide which ones warrant deep security review.\nFocus on entry points, authentication and authorization, network or process interactions, secrets handling, and other security-sensitive functionality.\nWhen uncertain, err on the side of including a file for further analysis.";
pub(crate) const FILE_TRIAGE_PROMPT_TEMPLATE: &str = "You will receive JSON objects describing candidate files from a repository. For each object, output a single JSON line with the same `id`, a boolean `include`, and a short `reason`.\n- Use include=true for files that likely influence production behaviour, handle user input, touch the network/filesystem, perform authentication/authorization, execute commands, or otherwise impact security.\n- Use include=false for files that are clearly documentation, tests, generated artefacts, or otherwise irrelevant to security review.\n\nReply with one JSON object per line in this exact form:\n{\"id\": <number>, \"include\": true|false, \"reason\": \"...\"}\n\nFiles:\n{files}";
