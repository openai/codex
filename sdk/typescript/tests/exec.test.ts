import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "@jest/globals";

import { CodexExec } from "../src/exec";

async function createExitExecutable(): Promise<{ filePath: string; dirPath: string }> {
  const dirPath = await fs.mkdtemp(path.join(os.tmpdir(), "codex-exec-test-"));
  const isWindows = process.platform === "win32";
  const fileName = isWindows ? "codex-exit.cmd" : "codex-exit";
  const filePath = path.join(dirPath, fileName);
  const contents = isWindows
    ? "@echo off\r\nfor /l %%i in (1,1,200) do @echo line %%i\r\nexit /b 2\r\n"
    : "#!/usr/bin/env node\nfor (let i = 0; i < 200; i += 1) {\n  process.stdout.write(`line ${i}\\n`);\n}\nprocess.exit(2);\n";
  await fs.writeFile(filePath, contents, { mode: 0o755 });
  if (!isWindows) {
    await fs.chmod(filePath, 0o755);
  }
  return { filePath, dirPath };
}

describe("CodexExec", () => {
  it("rejects promptly when the child exits before stdout is read", async () => {
    const { filePath, dirPath } = await createExitExecutable();

    try {
      const exec = new CodexExec(filePath);
      const controller = new AbortController();
      const runResultPromise = (async () => {
        for await (const _ of exec.run({ input: "hi", signal: controller.signal })) {
          await new Promise((resolve) => setTimeout(resolve, 2));
        }
        return { status: "resolved" as const };
      })().catch((error) => ({ status: "rejected" as const, error }));
      let timeoutId: NodeJS.Timeout | undefined;
      const timeout = new Promise<{ status: "timeout" }>((resolve) => {
        timeoutId = setTimeout(() => {
          controller.abort();
          resolve({ status: "timeout" });
        }, 2000);
      });
      const result = await Promise.race([runResultPromise, timeout]);
      if (timeoutId) {
        clearTimeout(timeoutId);
      }

      expect(result.status).toBe("rejected");
      if (result.status === "rejected") {
        expect(result.error).toBeInstanceOf(Error);
        expect(result.error.message).toMatch(/Codex Exec exited/);
      }
    } finally {
      await fs.rm(dirPath, { recursive: true, force: true });
    }
  });
});
