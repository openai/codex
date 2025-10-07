import assert from "node:assert/strict";
import { mkdtemp, copyFile, chmod } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";

import { buildPromptWithGuardrails } from "../src/cli.ts";
import { loadGuardrails } from "../src/extensions/guardrails.ts";

async function createTempProjectWithBridge() {
  const dir = await mkdtemp(path.join(tmpdir(), "codex-guardrails-"));
  const bridgeSourcePath = path.resolve("guardloop_bridge.py");
  const bridgeDestPath = path.join(dir, "guardloop_bridge.py");
  await copyFile(bridgeSourcePath, bridgeDestPath);
  await chmod(bridgeDestPath, "755");
  return { dir };
}

test("loadGuardrails returns code guardrails for a code prompt", async () => {
  const { dir } = await createTempProjectWithBridge();
  const codePrompt = "implement a function to sort a list";
  const guardrails = await loadGuardrails({ cwd: dir, prompt: codePrompt });

  const expectedGuardrails = [
    "## Guardrail: Code Standard",
    "- All functions must have a docstring.",
    "- Wrap async database calls in try-catch blocks.",
  ].join("\n");

  assert.strictEqual(guardrails, expectedGuardrails);
});

test("loadGuardrails returns no guardrails for a creative prompt", async () => {
  const { dir } = await createTempProjectWithBridge();
  const creativePrompt = "write a blog post about AI";
  const guardrails = await loadGuardrails({ cwd: dir, prompt: creativePrompt });

  assert.strictEqual(guardrails, "");
});

test("buildPromptWithGuardrails prepends guardrails correctly for a code prompt", async () => {
  const { dir } = await createTempProjectWithBridge();
  const userPrompt = "implement user authentication service";

  // We need to call the function that is actually exported and used.
  // The options object for buildPromptWithGuardrails needs to be constructed correctly.
  const result = await buildPromptWithGuardrails(userPrompt, {
    cwd: dir,
    guardrailsEnabled: true,
  });

  const expectedGuardrails = [
    "## Guardrail: Code Standard",
    "- All functions must have a docstring.",
    "- Wrap async database calls in try-catch blocks.",
  ].join("\n");

  assert.strictEqual(result, `${expectedGuardrails}\n\n${userPrompt}`);
});

test("buildPromptWithGuardrails returns only the prompt for a creative prompt", async () => {
  const { dir } = await createTempProjectWithBridge();
  const userPrompt = "write a poem about the sea";

  const result = await buildPromptWithGuardrails(userPrompt, {
    cwd: dir,
    guardrailsEnabled: true,
  });

  assert.strictEqual(result, userPrompt);
});