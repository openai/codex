import assert from "node:assert/strict";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";

import { buildPromptWithGuardrails } from "../src/cli.ts";
import { loadGuardrails } from "../src/extensions/guardrails.ts";

async function createTempProject() {
  const dir = await mkdtemp(path.join(tmpdir(), "codex-guardrails-"));
  const guardrailsDir = path.join(dir, ".guardrails");
  await mkdir(guardrailsDir, { recursive: true });
  return { dir, guardrailsDir };
}

test("loadGuardrails returns concatenated markdown", async () => {
  const { dir, guardrailsDir } = await createTempProject();
  await writeFile(path.join(guardrailsDir, "Test.md"), "Do not touch production.");

  const guardrails = await loadGuardrails({ cwd: dir });

  assert.match(guardrails, /## Guardrail: Test/);
  assert.match(guardrails, /Do not touch production\./);
});

test("buildPromptWithGuardrails prepends guardrails when enabled", async () => {
  const { dir, guardrailsDir } = await createTempProject();
  await writeFile(path.join(guardrailsDir, "A.md"), "Guardrail A");
  await writeFile(path.join(guardrailsDir, "B.md"), "Guardrail B");

  const guardrails = await loadGuardrails({ cwd: dir });
  const userPrompt = "Implement feature X";

  const withGuardrails = await buildPromptWithGuardrails(userPrompt, {
    guardrailsEnabled: true,
    cwd: dir,
  });

  assert.strictEqual(withGuardrails, `${guardrails}\n\n${userPrompt}`);

  const withoutGuardrails = await buildPromptWithGuardrails(userPrompt, {
    guardrailsEnabled: false,
    cwd: dir,
  });

  assert.strictEqual(withoutGuardrails, userPrompt);
});
