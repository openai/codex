pub(crate) const VALIDATION_PLAN_SYSTEM_PROMPT: &str = "You are an application security engineer planning minimal, safe validations for high-risk findings. Respond ONLY with JSON Lines as requested; do not include markdown or prose.";
pub(crate) const VALIDATION_PLAN_PROMPT_TEMPLATE: &str = r#"
Before any checks, create two test accounts if the app requires login. Prefer a short Python script that calls a signup endpoint or automates the registration form headlessly. If this is not feasible, return a `manual` instruction with a `login_url`.

Then validate the high-risk findings below. For each finding listed in Context, emit exactly one validation line keyed by its `id_kind`/`id_value`:
- If you can run a safe validation, choose the minimal tool and target.
- If you cannot safely validate (missing local target, unclear setup, requires production access), emit `tool:"none"` with a short `reason`.

Priorities (do not skip):
- HIGH-severity findings whose `TAXONOMY.vuln_tag` is one of: `idor`, `auth-bypass`, `authn-bypass`, `authz-bypass`, `missing-authz-check`, `sql-injection`, `xxe`, or `path-traversal-*`.
- Findings with `verification_types` including `crash_poc` (treat as memory corruption / crash-class validation).

For each validation, choose the minimal tool and target:
- Use the Playwright MCP tool for web_browser checks (supply a reachable URL in `target`).
- Use tool "curl" for network_api checks (supply full URL in `target`).
- Use tool "python" only if a short, non-destructive PoC is essential (include inline script text in `script`).

Target safety rules:
- For `curl`/`playwright`, prefer a local target from a locally built binary or local Docker container (e.g., `http://localhost:<port>/...`). Do not target production or unknown remote hosts.

For python validations, the script must:
- Print a step-by-step log with clear section headers (at least `CONTROL` and `TRIGGER`) so the run can be pasted into the security report.
- Run a control action first (benign request/input) and then the trigger action, and print real stdout/stderr outputs for both.
- Exit 0 only when the expected security-relevant signal is observed; otherwise exit non-zero.

Special case: memory corruption
- If `verification_types` includes `crash_poc` or the finding looks like a memory corruption bug in native code (C/C++ decoder, image preprocessing, FFI boundary, etc.), validate that this bug exist by testing it against a asan-compiled version of the program.

Rules:
- Keep requests minimal and non-destructive; no state-changing actions.
- Prefer headless checks (e.g., page loads, HTTP status, presence of a marker string).
- Max 5 requests total; prioritize Critical/High severity or lowest risk_rank.

Context (findings):
{findings}

Output format (one JSON object per line, no fences):
- For account setup (emit at most one line): {"id_kind":"setup","action":"register|manual","login_url":"<string, optional>","tool":"python|manual","script":"<string, optional>"}
- For validations: {"id_kind":"risk_rank|summary_id","id_value":<int>,"tool":"playwright|curl|python|none","target":"<string, optional>","script":"<string, optional>","reason":"<string, optional>"}
"#;

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
