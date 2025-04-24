import {
  ActionType,
  apply_commit,
  assemble_changes,
  DiffError,
  identify_files_added,
  identify_files_needed,
  load_files,
  patch_to_commit,
  process_patch,
  text_to_patch,
} from "../src/utils/agent/apply-patch.js";
import { test, expect } from "vitest";

function createInMemoryFS(initialFiles: Record<string, string>) {
  const files: Record<string, string> = { ...initialFiles };
  const writes: Record<string, string> = {};
  const removals: Array<string> = [];

  const openFn = (p: string): string => {
    const file = files[p];
    if (typeof file === "string") {
      return file;
    } else {
      throw new Error(`File not found: ${p}`);
    }
  };

  const writeFn = (p: string, content: string): void => {
    files[p] = content;
    writes[p] = content;
  };

  const removeFn = (p: string): void => {
    delete files[p];
    removals.push(p);
  };

  return { openFn, writeFn, removeFn, writes, removals, files };
}

test("process_patch - update file", () => {
  const patch = `*** Begin Patch
*** Update File: a.txt
@@
-hello
+hello world
*** End Patch`;

  const fs = createInMemoryFS({ "a.txt": "hello" });

  const result = process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(result).toBe("Done!");
  expect(fs.writes).toEqual({ "a.txt": "hello world" });
  expect(fs.removals).toEqual([]);
});

// ---------------------------------------------------------------------------
// Unicode canonicalisation tests – hyphen / dash / quote look-alikes
// ---------------------------------------------------------------------------

test("process_patch tolerates hyphen/dash variants", () => {
  // The file contains EN DASH (\u2013) and NO-BREAK HYPHEN (\u2011)
  const original =
    "first\nimport foo  # local import \u2013 avoids top\u2011level dep\nlast";

  const patch = `*** Begin Patch\n*** Update File: uni.txt\n@@\n-import foo  # local import - avoids top-level dep\n+import foo  # HANDLED\n*** End Patch`;

  const fs = createInMemoryFS({ "uni.txt": original });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.files["uni.txt"]!.includes("HANDLED")).toBe(true);
});

test.skip("process_patch tolerates smart quotes", () => {
  const original = "console.log(\u201Chello\u201D);"; // “hello” with smart quotes

  const patch = `*** Begin Patch\n*** Update File: quotes.js\n@@\n-console.log(\\"hello\\");\n+console.log(\\"HELLO\\");\n*** End Patch`;

  const fs = createInMemoryFS({ "quotes.js": original });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.files["quotes.js"]).toBe('console.log("HELLO");');
});

test("process_patch - add file", () => {
  const patch = `*** Begin Patch
*** Add File: b.txt
+new content
*** End Patch`;

  const fs = createInMemoryFS({});

  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.writes).toEqual({ "b.txt": "new content" });
  expect(fs.removals).toEqual([]);
});

test("process_patch - delete file", () => {
  const patch = `*** Begin Patch
*** Delete File: c.txt
*** End Patch`;

  const fs = createInMemoryFS({ "c.txt": "to be removed" });

  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.writes).toEqual({});
  expect(fs.removals).toEqual(["c.txt"]);
});

test("identify_files_needed & identify_files_added", () => {
  const patch = `*** Begin Patch
*** Update File: a.txt
*** Delete File: b.txt
*** Add File: c.txt
*** End Patch`;

  expect(identify_files_needed(patch).sort()).toEqual(
    ["a.txt", "b.txt"].sort(),
  );
  expect(identify_files_added(patch)).toEqual(["c.txt"]);
});

test("process_patch - update file with multiple chunks", () => {
  const original = "line1\nline2\nline3\nline4";
  const patch = `*** Begin Patch
*** Update File: multi.txt
@@
 line1
-line2
+line2 updated
 line3
+inserted line
 line4
*** End Patch`;

  const fs = createInMemoryFS({ "multi.txt": original });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  const expected = "line1\nline2 updated\nline3\ninserted line\nline4";
  expect(fs.writes).toEqual({ "multi.txt": expected });
  expect(fs.removals).toEqual([]);
});

test("process_patch - move file (rename)", () => {
  const patch = `*** Begin Patch
*** Update File: old.txt
*** Move to: new.txt
@@
-old
+new
*** End Patch`;

  const fs = createInMemoryFS({ "old.txt": "old" });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.writes).toEqual({ "new.txt": "new" });
  expect(fs.removals).toEqual(["old.txt"]);
});

test("process_patch - combined add, update, delete", () => {
  const patch = `*** Begin Patch
*** Add File: added.txt
+added contents
*** Update File: upd.txt
@@
-old value
+new value
*** Delete File: del.txt
*** End Patch`;

  const fs = createInMemoryFS({
    "upd.txt": "old value",
    "del.txt": "delete me",
  });

  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.writes).toEqual({
    "added.txt": "added contents",
    "upd.txt": "new value",
  });
  expect(fs.removals).toEqual(["del.txt"]);
});

test("process_patch - readme edit", () => {
  const original = `
#### Fix an issue

\`\`\`sh
# First, copy an error
# Then, start codex with interactive mode
codex

# Or you can pass in via command line argument
codex "Fix this issue: $(pbpaste)"

# Or even as a task (it should use your current repo and branch)
codex -t "Fix this issue: $(pbpaste)"
\`\`\`
`;
  const patch = `*** Begin Patch
*** Update File: README.md
@@
  codex -t "Fix this issue: $(pbpaste)"
  \`\`\`
+
+hello
*** End Patch`;
  const expected = `
#### Fix an issue

\`\`\`sh
# First, copy an error
# Then, start codex with interactive mode
codex

# Or you can pass in via command line argument
codex "Fix this issue: $(pbpaste)"

# Or even as a task (it should use your current repo and branch)
codex -t "Fix this issue: $(pbpaste)"
\`\`\`

hello
`;

  const fs = createInMemoryFS({ "README.md": original });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);

  expect(fs.writes).toEqual({ "README.md": expected });
});

test("process_patch - invalid patch throws DiffError", () => {
  const patch = `*** Begin Patch
*** Update File: missing.txt
@@
+something
*** End Patch`;

  const fs = createInMemoryFS({});

  expect(() =>
    process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn),
  ).toThrow(DiffError);
});

test("process_patch - tolerates omitted space for keep line", () => {
  const original = "line1\nline2\nline3";
  const patch = `*** Begin Patch\n*** Update File: foo.txt\n@@\n line1\n-line2\n+some new line2\nline3\n*** End Patch`;
  const fs = createInMemoryFS({ "foo.txt": original });
  process_patch(patch, fs.openFn, fs.writeFn, fs.removeFn);
  expect(fs.files["foo.txt"]).toBe("line1\nsome new line2\nline3");
});

test("assemble_changes correctly detects add, update and delete", () => {
  const orig = {
    "a.txt": "old",
    "b.txt": "keep",
    "c.txt": "remove",
  };
  const updated = {
    "a.txt": "new", // update
    "b.txt": "keep", // unchanged – should be ignored
    "c.txt": undefined as unknown as string, // delete
    "d.txt": "created", // add
  };

  const commit = assemble_changes(orig, updated).changes;

  expect(commit["a.txt"]).toEqual({
    type: ActionType.UPDATE,
    old_content: "old",
    new_content: "new",
  });
  expect(commit["c.txt"]).toEqual({
    type: ActionType.DELETE,
    old_content: "remove",
  });
  expect(commit["d.txt"]).toEqual({
    type: ActionType.ADD,
    new_content: "created",
  });

  // unchanged files should not appear in commit
  expect(commit).not.toHaveProperty("b.txt");
});

test("text_to_patch + patch_to_commit handle update and add", () => {
  const originalFiles = {
    "a.txt": "old line",
  };

  const patch = `*** Begin Patch
*** Update File: a.txt
@@
-old line
+new line
*** Add File: b.txt
+content new
*** End Patch`;

  const [parsedPatch] = text_to_patch(patch, originalFiles);
  const commit = patch_to_commit(parsedPatch, originalFiles).changes;

  expect(commit["a.txt"]).toEqual({
    type: ActionType.UPDATE,
    old_content: "old line",
    new_content: "new line",
  });
  expect(commit["b.txt"]).toEqual({
    type: ActionType.ADD,
    new_content: "content new",
  });
});

test("load_files throws DiffError when file is missing", () => {
  const { openFn } = createInMemoryFS({ "exists.txt": "hi" });
  // intentionally include a missing file in the list
  expect(() => load_files(["exists.txt", "missing.txt"], openFn)).toThrow(
    DiffError,
  );
});

test("apply_commit correctly performs move / rename operations", () => {
  const commit = {
    changes: {
      "old.txt": {
        type: ActionType.UPDATE,
        old_content: "old",
        new_content: "new",
        move_path: "new.txt",
      },
    },
  };

  const { writeFn, removeFn, writes, removals } = createInMemoryFS({});

  apply_commit(commit, writeFn, removeFn);

  expect(writes).toEqual({ "new.txt": "new" });
  expect(removals).toEqual(["old.txt"]);
});
