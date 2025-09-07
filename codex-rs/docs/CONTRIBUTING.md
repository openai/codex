Contributing — Resolver Help Golden

Why
- We keep a golden snapshot of the resolver’s `--help` output to prevent accidental drift and flaky CI.

How to update the golden
- After intentional changes to `scripts/resolve_safe_sync.sh --help` wording or formatting:
  - Run the normalized pipeline (same as tests):
    - `just update-resolver-golden`
    - or `LC_ALL=C LANG=C bash scripts/resolve_safe_sync.sh --help | tr -d '\r' | awk 'NF{print $0}' ORS='\n' > docs/golden/resolver_help.txt`
- Commit the updated `docs/golden/resolver_help.txt` alongside your changes.

Notes
- The help is formatted with single blank-line separators and a trailing newline. The script includes a `--self-test-help` to validate this locally (`bash scripts/resolve_safe_sync.sh --self-test-help`).
- CI validates both the exit-code phrases and the golden snapshot across Linux/macOS/Windows.
