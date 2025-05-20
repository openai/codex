import { execSync, execFileSync } from "node:child_process";

// The objects thrown by `child_process.execSync()` are `Error` instances that
// include additional, undocumented properties such as `status` (exit code) and
// `stdout` (captured standard output). Declare a minimal interface that captures
// just the fields we need so that we can avoid the use of `any` while keeping
// the checks type-safe.
interface ExecSyncError extends Error {
  // Exit status code. When a diff is produced, git exits with code 1 which we
  // treat as a non-error signal.
  status?: number;
  // Captured stdout. We rely on this to obtain the diff output when git exits
  // with status 1.
  stdout?: string;
}

// Type-guard that narrows an unknown value to `ExecSyncError`.
function isExecSyncError(err: unknown): err is ExecSyncError {
  return (
    typeof err === "object" && err != null && "status" in err && "stdout" in err
  );
}

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
    // diff for tracked files **and** include any untracked files so that the
    // `/diff` overlay shows a complete picture of the working tree state.

    // 1. Diff for tracked files (unchanged behaviour)
    let trackedDiff = "";
    try {
      trackedDiff = execSync("git diff --color", {
        encoding: "utf8",
        maxBuffer: 10 * 1024 * 1024, // 10 MB ought to be enough for now
      });
    } catch (err) {
      // Exit status 1 simply means that differences were found. Capture the
      // diff from stdout in that case. Re-throw for any other status codes.
      if (
        isExecSyncError(err) &&
        err.status === 1 &&
        typeof err.stdout === "string"
      ) {
        trackedDiff = err.stdout;
      } else {
        throw err;
      }
    }

    // 2. Determine untracked files.
    //    We use `git ls-files --others --exclude-standard` which outputs paths
    //    relative to the repository root, one per line. These are files that
    //    are not tracked *and* are not ignored by .gitignore.
    const untrackedOutput = execSync(
      "git ls-files --others --exclude-standard",
      {
        encoding: "utf8",
        maxBuffer: 10 * 1024 * 1024,
      },
    );

    const untrackedFiles = untrackedOutput
      .split("\n")
      .map((p) => p.trim())
      .filter(Boolean);

    let untrackedDiff = "";

    const nullDevice = process.platform === "win32" ? "NUL" : "/dev/null";

    for (const file of untrackedFiles) {
      try {
        // `git diff --no-index` produces a diff even outside the index by
        // comparing two paths. We compare the file against /dev/null so that
        // the file is treated as "new".
        //
        // `git diff --color --no-index /dev/null <file>` exits with status 1
        // when differences are found, so we capture stdout from the thrown
        // error object instead of letting it propagate. Using `execFileSync`
        // avoids shell interpolation issues with special characters in the
        // path.
        execFileSync(
          "git",
          ["diff", "--color", "--no-index", "--", nullDevice, file],
          {
            encoding: "utf8",
            stdio: ["ignore", "pipe", "ignore"],
            maxBuffer: 10 * 1024 * 1024,
          },
        );
      } catch (err) {
        if (
          isExecSyncError(err) &&
          // Exit status 1 simply means that the two inputs differ, which is
          // exactly what we expect here. Any other status code indicates a
          // real error (e.g. the file disappeared between the ls-files and
          // diff calls), so re-throw those.
          err.status === 1 &&
          typeof err.stdout === "string"
        ) {
          untrackedDiff += err.stdout;
        } else {
          throw err;
        }
      }
    }

    // Concatenate tracked and untracked diffs.
    const combinedDiff = `${trackedDiff}${untrackedDiff}`;

    return { isGitRepo: true, diff: combinedDiff };
  } catch {
    // Either git is not installed or we’re not inside a repository.
    return { isGitRepo: false, diff: "" };
  }
}
