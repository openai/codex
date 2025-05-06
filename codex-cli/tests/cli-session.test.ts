import { spawnSync, execSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";

// Resolve __dirname in ESM
const __dirname = path.dirname(fileURLToPath(import.meta.url));
// CLI entrypoint
const cliPath = path.resolve(__dirname, "../bin/codex.js");

import {
  afterEach,
  beforeEach,
  describe,
  expect,
  test,
  beforeAll,
} from "vitest";

// Build the CLI bundle once before tests
const cliRoot = path.resolve(__dirname, "..");
beforeAll(() => {
  execSync("npm run build", { cwd: cliRoot, stdio: "inherit" });
});
// Helper to invoke codex CLI with a clean HOME
function runCli(args: Array<string>, homeDir: string) {
  const env = { ...process.env, HOME: homeDir };
  return spawnSync(process.execPath, [cliPath, ...args], {
    env,
    cwd: path.resolve(__dirname, ".."),
    encoding: "utf8",
  });
}

describe("codex session CLI", () => {
  let tmpHome: string;
  const sessionsDirName = path.join(".codex", "sessions");

  beforeEach(() => {
    tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), "codex-session-"));
  });
  afterEach(() => {
    fs.rmSync(tmpHome, { recursive: true, force: true });
  });

  test("session-list shows nothing when no sessions exist", () => {
    const result = runCli(["--session-list"], tmpHome);
    expect(result.status).toBe(0);
    expect(result.stdout).toBe("");
  });

  test("session management: list, path, dump, delete", () => {
    // Prepare a dummy session file
    const sessId = "testsess";
    const sessDir = path.join(tmpHome, sessionsDirName);
    fs.mkdirSync(sessDir, { recursive: true });
    const filename = `${sessId}.json`;
    const filePath = path.join(sessDir, filename);
    const fileContent = JSON.stringify(
      {
        session: { id: sessId, timestamp: "", instructions: "" },
        items: [
          {
            id: "1",
            type: "message",
            role: "assistant",
            content: [{ type: "output_text", text: "hello" }],
          },
        ],
      },
      null,
      2,
    );
    fs.writeFileSync(filePath, fileContent, "utf8");

    // list
    const list = runCli(["--session-list"], tmpHome);
    expect(list.status).toBe(0);
    expect(list.stdout.trim()).contain(sessId);

    // path
    const p = runCli(["--session-path", sessId], tmpHome);
    expect(p.status).toBe(0);
    expect(p.stdout.trim()).toBe(filePath);

    // dump
    const dump = runCli(["--session-dump", sessId], tmpHome);
    expect(dump.status).toBe(0);
    // console.log adds a trailing newline
    expect(dump.stdout).toBe(fileContent + "\n");

    // delete
    const del = runCli(["--session-delete", sessId], tmpHome);
    expect(del.status).toBe(0);

    // Check that the session file is deleted
    const listAfterDelete = runCli(["--session-list"], tmpHome);
    expect(listAfterDelete.status).toBe(0);
    expect(listAfterDelete.stdout.trim()).not.contain(sessId);
  });
});
