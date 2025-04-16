import { quote } from "shell-quote";

/**
 * Format the args of an exec command for display as a single string. Prefer
 * this to doing `args.join(" ")` as this will handle quoting and escaping
 * correctly. See unit test for details.
 */
export function formatCommandForDisplay(command: Array<string>): string {
  // The model often wraps arbitrary shell commands in an invocation that looks
  // like:
  //
  //   ["bash", "-lc", "'<actual command>'"]
  //
  // When displaying these back to the user, we do NOT want to show the
  // boiler‑plate "bash -lc" wrapper. Instead, we want to surface only the
  // actual command that bash will evaluate.

  // Historically we detected this by first quoting the entire command array
  // with `shell‑quote` and then using a regular expression to peel off the
  // `bash -lc '…'` prefix. However, that approach was brittle (it depended on
  // the exact quoting behavior of `shell-quote`) and unnecessarily
  // inefficient.

  // A simpler and more robust approach is to look at the raw command array
  // itself. If it matches the shape produced by our exec helpers—exactly three
  // entries where the first two are «bash» and «-lc»—then we can return the
  // third entry directly (after stripping surrounding single quotes if they
  // are present).

  try {
    if (
      command.length === 3 &&
      command[0] === "bash" &&
      command[1] === "-lc" &&
      typeof command[2] === "string"
    ) {
      let inner = command[2];

      // Some callers wrap the actual command in single quotes (e.g. `'echo foo'`).
      // For display purposes we want to drop those outer quotes so that the
      // rendered command looks exactly like what the user typed.
      if (inner.startsWith("'") && inner.endsWith("'")) {
        inner = inner.slice(1, -1);
      }

      return inner;
    }

    return quote(command);
  } catch (err) {
    return command.join(" ");
  }
}
