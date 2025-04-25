import TextBuffer from "../src/text-buffer";
import { describe, it, expect, vi } from "vitest";

/* -------------------------------------------------------------------------
 *  External $EDITOR integration – behavioural contract
 * ---------------------------------------------------------------------- */

describe("TextBuffer – open in external $EDITOR", () => {
  it("replaces the buffer with the contents saved by the editor", async () => {
    // Initial text put into the file.
    const initial = [
      "// TODO: draft release notes",
      "",
      "* Fixed memory leak in xyz module.",
    ].join("\n");

    const buf = new TextBuffer(initial);

    // -------------------------------------------------------------------
    //  Stub the child_process.spawnSync call so no real editor launches.
    // -------------------------------------------------------------------
    const mockSpawn = vi
      .spyOn(require("node:child_process"), "spawnSync")
      .mockImplementation((_cmd, args: any) => {
        const argv = args as Array<string>;
        const file = argv[argv.length - 1];
        // Lazily append a dummy line – our faux "edit".
        require("node:fs").appendFileSync(
          file,
          "\n* Added unit tests for external editor integration.",
        );
        return { status: 0 } as any;
      });

    try {
      await buf.openInExternalEditor({ editor: "nano" }); // editor param ignored in stub
    } finally {
      mockSpawn.mockRestore();
    }

    const want = [
      "// TODO: draft release notes",
      "",
      "* Fixed memory leak in xyz module.",
      "* Added unit tests for external editor integration.",
    ].join("\n");

    expect(buf.getText()).toBe(want);
    // Cursor should land at the *end* of the newly imported text.
    const [row, col] = buf.getCursor();
    expect(row).toBe(3); // 4th line (0‑based)
    expect(col).toBe(
      "* Added unit tests for external editor integration.".length,
    );
  });
});
