import { execSync } from "child_process";

/**
 * Returns true if the given directory is part of a Git repository.
 *
 * This uses the canonical Git command `git rev-parse --is-inside-work-tree`
 * which exits with status 0 when executed anywhere inside a working tree
 * (including the repo root) and exits with a non‑zero status otherwise. We
 * intentionally ignore stdout/stderr and only rely on the exit code so that
 * this works consistently across Git versions and configurations.
 *
 * The function is fully synchronous because it is typically used during CLI
 * startup (e.g. to decide whether to enable certain Git‑specific features) and
 * a synchronous check keeps such call‑sites simple. The command is extremely
 * fast (~1ms) so blocking the event‑loop briefly is acceptable.
 */
export function checkInGit(workdir: string): boolean {
  try {
    // "git rev-parse --is-inside-work-tree" prints either "true" or "false" to
    // stdout. We don't care about the output — only the exit status — so we
    // discard stdio for maximum performance and to avoid leaking noise if the
    // caller happens to inherit stdio.
    execSync("git rev-parse --is-inside-work-tree", {
      cwd: workdir,
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}
