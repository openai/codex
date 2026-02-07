pub(crate) const BUG_RERANK_SYSTEM_PROMPT: &str = "You are a senior application security engineer triaging review findings. Reassess customer-facing risk using the supplied specification/threat model context (no full file listings). Only respond with JSON Lines.";
pub(crate) const BUG_RERANK_PROMPT_TEMPLATE: &str = r#"
Specification/threat model excerpt (trimmed; pull in concrete details or note if unavailable):
{spec_excerpt}

Examples:
- External unauthenticated remote code execution on a production API ⇒ risk_score 95, severity "High", reason "unauth RCE takeover".
- Memory corruption reachable from untrusted inputs in a local daemon ⇒ risk_score 85, severity "High", reason "untrusted input RCE".
- Stored XSS on user dashboards that leaks session tokens ⇒ risk_score 72, severity "High", reason "persistent session theft".
- Originally escalated CSRF on an internal admin tool behind SSO ⇒ risk_score 28, severity "Low", reason "internal-only with SSO".
- Header injection in a deprecated endpoint with response sanitization ⇒ risk_score 18, severity "Informational", reason "sanitized legacy endpoint".
- Static analysis high alert that only touches dead code ⇒ risk_score 0, severity "Ignore", reason "dead code path".
- High-severity SQL injection finding that uses fully parameterized queries ⇒ risk_score 20, severity "Low", reason "parameterized queries".
- SSRF flagged as critical but the target requires internal metadata access tokens ⇒ risk_score 24, severity "Low", reason "internal metadata token".
- Signature-verification bypass in a CLI used by production automation ⇒ risk_score 78, severity "High", reason "automation trust bypass".
- Reported secret leak found in sample dev config with rotate-on-startup hook ⇒ risk_score 12, severity "Informational", reason "sample config only".

# Available tools
- READ: respond with `READ: <relative path>#Lstart-Lend` (range optional) to inspect specific source code.
- SEARCH: respond with `SEARCH: literal:<term>` or `SEARCH: regex:<pattern>` to run ripgrep over the repository root (returns colored matches with line numbers).
- GREP_FILES: respond with `GREP_FILES: {"pattern":"needle","include":"*.rs","path":"subdir","limit":200}` to list files whose contents match, ordered by modification time.
- READ/SEARCH/GREP_FILES are enabled in this rerank step; use them when evidence is missing.
- Issue at most one tool command per round and wait for the tool output before continuing. Reuse earlier tool outputs when possible.

Instructions:
- Output severity **only** from ["High","Medium","Low","Informational","Ignore"]. Map "critical"/"p0" to "High".
- Produce `risk_score` between 0-100 (higher means greater customer impact) and use the full range for comparability.
- Review the spec excerpt, blame metadata, and file locations before requesting anything new; reuse existing context attachments when possible.
- If real-world usage is unclear (especially for third-party libraries, CLIs, protocols, or crypto/auth flows), use READ/SEARCH/GREP_FILES to confirm typical usage paths before final reranking.
- If available in your runtime, use web search to confirm common usage patterns or adoption by other projects/organizations and incorporate that evidence into likelihood/impact scoring.
- Do not down-rank findings solely because the entrypoint is a CLI or crypto-related code; rerank based on deployment reachability, caller control, and blast radius.
- Determine severity from Impact and Likelihood levels using this deterministic risk matrix:
  - Convert levels to numbers: High=3, Medium=2, Low=1.
  - risk = Impact * Likelihood (range 1-9).
  - 6-9 => severity "High"; 3-4 => "Medium"; 1-2 => "Low".
  - If the finding's `impact`/`likelihood` fields include explicit levels (e.g., start with "High - ..."), use those; otherwise infer levels from the description and state uncertainty in the reason.
- For memory-corruption findings, calibrate `risk_score` using this baseline and then adjust for reachability/blast radius:
  - 60: Uncontrolled or incidental memory read (random bytes/offset; attacker lacks target/size control).
  - 70: Uncontrolled or limited memory write (write triggered but target/payload not precisely controlled).
  - 80: Controllable memory read (attacker controls address/size; secrets demonstrably exfiltrated) OR controllable memory write (targeted write to chosen address/structure).
- If you still lack certainty, request concrete follow-up (e.g., repo_search, read_file, git blame) in the reason and cite the spec section you need.
- Reference concrete evidence (spec section, tool name, log line) in the reason when you confirm mitigations or reclassify a finding.
- Prefer reusing existing tool outputs and cached specs before launching new expensive calls; only request fresh tooling when the supplied artifacts truly lack the needed context.
- Prioritize issues that meaningfully threaten infrastructure control, user data, or financial integrity; make the impact explicit in the reason.
- Down-rank issues when mitigations or limited blast radius materially reduce customer risk, even if the initial triage labeled them "High". For DoS, only consider elevated risk when autoscaling or graceful recovery cannot mitigate an availability loss that causes real user/data impact.
- De-emphasize DoS findings that are just repeated calls without resource amplification (memory/CPU/locks); if no meaningful exhaustion or lockout risk exists, ignore the finding.
- Upgrade issues when exploitability or exposure was understated, or when multiple components amplify the blast radius.
- Respond with one JSON object per finding, **in the same order**, formatted exactly as:
  {{"id": <number>, "risk_score": <0-100>, "severity": "<High|Medium|Low|Informational|Ignore>", "reason": "<concise reason>"}}

Findings:
{findings}
"#;
