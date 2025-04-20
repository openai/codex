import { execSync } from "node:child_process";

/**
 * Returns the current Git diff for the working directory. If the current
 * working directory is not inside a Git repository, `isGitRepo` will be
 * false and `diff` will be an empty string.
 */
export function getGitDiff(): {
  isGitRepo: boolean;
  diff: string;
} {
  try {
    // First check whether we are inside a git repository. `rev‑parse` exits
    // with a non‑zero status code if not.
    execSync("git rev-parse --is-inside-work-tree", { stdio: "ignore" });

    // If the above call didn’t throw, we are inside a git repo. Retrieve the
    // diff including color codes so that the overlay can render them.
    const output = execSync("git diff --color", {
      encoding: "utf8",
      maxBuffer: 10 * 1024 * 1024, // 10 MB ought to be enough for now
    });

    return { isGitRepo: true, diff: output };
  } catch {
    // Either git is not installed or we’re not inside a repository.
    return { isGitRepo: false, diff: "" };
  }
}
