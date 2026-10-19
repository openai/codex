import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { fetchCodexManual } from "../src/assets/samples/openai-docs/scripts/fetch-codex-manual.mjs";

const execFileAsync = promisify(execFile);
const scripts = new URL(
  "../src/assets/samples/openai-docs/scripts/",
  import.meta.url,
);
const resolverScript = fileURLToPath(
  new URL("resolve-latest-model-info.cjs", scripts),
);
const manual = "# Codex manual\n\n## Getting started\nUse Codex.\n";
const hash = createHash("sha256").update(manual).digest("hex");
const markdown =
  "latestModelInfo:\n  model: test-model\n  migrationGuide: /migration\n  promptingGuide: /prompting\n";

for (const failure of ["timeout", "truncated body"]) {
  test(`manual ${failure} falls back to curl before caching`, async (t) => {
    // Top-level tests run sequentially; restore proxy settings after each case.
    for (const [key, value] of Object.entries(process.env)) {
      if (/^(https?|all)_proxy$/i.test(key)) {
        delete process.env[key];
        t.after(() => {
          process.env[key] = value;
        });
      }
    }
    const cacheDir = await mkdtemp(path.join(tmpdir(), "docs-download-test-"));
    t.after(() => rm(cacheDir, { recursive: true, force: true }));
    const requests = [];
    const server = createServer((request, response) => {
      requests.push(request.method);
      response.writeHead(200, { "x-content-sha256": hash });
      response.end(manual);
    });
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    t.after(() => new Promise((resolve) => server.close(resolve)));
    const url = `http://127.0.0.1:${server.address().port}/manual`;
    const fetchMock = t.mock.method(
      globalThis,
      "fetch",
      async (_url, { method, signal }) => ({
        ok: true,
        status: 200,
        headers: new Headers({ "x-content-sha256": hash }),
        text: async () => {
          assert.notEqual(method, "HEAD", "HEAD body must not be consumed");
          if (failure === "truncated body") throw new Error("truncated body");
          // Bounded success makes the old cleared timeout fail without hanging.
          await delay(500, null, { signal });
          return manual;
        },
      }),
    );
    const result = await fetchCodexManual({
      manualUrl: url,
      cacheDir,
      timeoutMs: 25,
    });
    assert.deepEqual(
      fetchMock.mock.calls.map((call) => call.arguments[1].method),
      ["HEAD", "GET"],
    );
    assert.equal(result.status.cacheStatus, "updated");
    assert.deepEqual(requests, ["GET"]);
    assert.equal(
      await readFile(path.join(cacheDir, "codex-manual.md"), "utf8"),
      manual,
    );
  });
}

for (const failures of [1, 3]) {
  test(`model resolver ${failures === 1 ? "retries a failed body read" : "stops after three failed body reads"}`, async () => {
    const program = `
      let calls = 0;
      globalThis.fetch = async () => ({
        ok: true, status: 200,
        text: async () => {
          if (++calls <= ${failures}) throw new Error("truncated body");
          return ${JSON.stringify(markdown)};
        },
      });
      process.on("exit", () => console.error("CALLS=" + calls));
      process.argv = [process.execPath, ${JSON.stringify(resolverScript)}, "--source", "https://example.invalid/model"];
      require(${JSON.stringify(resolverScript)});
    `;
    const result = await execFileAsync(process.execPath, ["-e", program], {
      timeout: 5000,
    }).then(
      (result) => ({ ...result, code: 0 }),
      (error) => error,
    );
    assert.equal(result.code, failures === 1 ? 0 : 1, result.stderr);
    assert.equal(
      Number(/CALLS=(\d+)/.exec(result.stderr)?.[1]),
      failures === 1 ? 2 : 3,
    );
    if (failures === 1) {
      assert.equal(JSON.parse(result.stdout).model, "test-model");
    } else {
      assert.match(result.stderr, /truncated body/);
    }
  });
}
