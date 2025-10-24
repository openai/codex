## Overview
`linux_sandbox::landlock` installs per-thread sandbox restrictions before exec’ing the target command. It combines Landlock filesystem rules with seccomp network filters so spawned commands inherit the policy without affecting the parent process.

## Detailed Behavior
- `apply_sandbox_policy_to_current_thread`:
  - Checks `SandboxPolicy` capabilities. When network access is restricted, calls `install_network_seccomp_filter_on_current_thread` to forbid non-AF_UNIX sockets and other networking syscalls.
  - When disk write access is restricted, translates the policy’s writable roots (relative to the provided cwd) into absolute paths and passes them to `install_filesystem_landlock_rules_on_current_thread`.
  - TODO notes future work for read-only enforcement.
- `install_filesystem_landlock_rules_on_current_thread`:
  - Builds a Landlock ruleset (ABI v5) granting read access to `/` and `/dev/null`, write access only to whitelisted roots, and `no_new_privs`.
  - Applies the ruleset and returns `CodexErr::Sandbox` if enforcement fails.
- `install_network_seccomp_filter_on_current_thread`:
  - Uses `seccompiler` to build a filter denying most socket-related syscalls (connect, bind, send, etc.), while allowing AF_UNIX sockets. Applies the BPF program via `seccompiler::apply_filter`.
  - Returns `SandboxErr` on failure, which `apply_sandbox_policy_to_current_thread` lifts into `CodexErr`.

## Broader Context
- `linux_run_main::run_main` invokes this module before exec’ing the intended command. `SandboxPolicy` originates from Codex core and reflects CLI overrides (`--sandbox`, `--full-auto`, etc.).
- Errors surface through panics or `CodexErr::Sandbox`, helping codex-exec report policy misconfigurations or unsupported platforms.
- Context can't yet be determined for read-access restrictions; future sandbox enhancements will extend this module.

## Technical Debt
- Read restriction TODO remains unimplemented; when policies require read confinement, this module must expand Landlock coverage or combine additional seccomp rules.
- Network filter currently allows `recvfrom` unconditionally (documented rationale); reassessing this exception as tool requirements evolve would tighten security.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Implement read-access restrictions for policies that disallow full disk read access.
    - Revisit the networking exceptions (e.g., allowing `recvfrom`) to ensure they align with the minimal necessary syscall set.
related_specs:
  - ./linux_run_main.rs.spec.md
  - ../core/src/sandboxing.rs.spec.md
