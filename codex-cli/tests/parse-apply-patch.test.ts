import { parseApplyPatch } from "../src/parse-apply-patch";
import { expect, test, describe } from "vitest";

// Helper function to unwrap a non‑null result in tests that expect success.
function mustParse(patch: string) {
  const parsed = parseApplyPatch(patch);
  if (parsed == null) {
    throw new Error(
      "Expected patch to be valid, but parseApplyPatch returned null",
    );
  }
  return parsed;
}

describe("parseApplyPatch", () => {
  test("parses create, update and delete operations in a single patch", () => {
    const patch = `*** Begin Patch\n*** Add File: created.txt\n+hello\n+world\n*** Update File: updated.txt\n@@\n-old\n+new\n*** Delete File: removed.txt\n*** End Patch`;

    const ops = mustParse(patch);

    expect(ops).toEqual([
      {
        type: "create",
        path: "created.txt",
        content: "hello\nworld",
      },
      {
        type: "update",
        path: "updated.txt",
        update: "@@\n-old\n+new",
        added: 1,
        deleted: 1,
      },
      {
        type: "delete",
        path: "removed.txt",
      },
    ]);
  });

  test("returns null for an invalid patch (missing prefix)", () => {
    const invalid = `*** Add File: foo.txt\n+bar\n*** End Patch`;
    expect(parseApplyPatch(invalid)).toBeNull();
  });
});
