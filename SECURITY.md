# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 1.x     | Yes (security and critical bug fixes) |
| 0.x     | No |

## Reporting a Vulnerability

- Please report privately via GitHub Security Advisories (Security > Report a vulnerability). This keeps details out of public issues until a fix is available.
- Include: affected version or commit, environment, reproduction steps or proof of concept, expected impact, and any relevant logs (scrub secrets first).
- Response targets: acknowledgement within 72 hours; initial assessment within 7 days; fix or mitigation ETA provided after triage. Critical issues are prioritized for hotfix releases.
- If you cannot use the advisory form, contact the maintainer privately (email or direct message). Do not post exploits in public issues or PRs.
- If the issue lies in a dependency, please also notify the upstream project while letting us know how it affects this codebase.
- If credentials or keys may be exposed, rotate them immediately and note the rotation steps in your report.

## Security Hardening and Testing

- We run automated dependency audits and linting; please include dependency names and versions if they appear to be the cause.
- For proposed fixes, avoid submitting exploit details in public PR descriptions; link the advisory or share the details privately instead.
