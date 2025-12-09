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
