import path from "node:path";
import { execFileSync } from "node:child_process";

/**
 * Load guardrails from the GuardLoop bridge.
 *
 * @param {{ cwd?: string, prompt?: string }} [options]
 * @returns {Promise<string>} Combined guardrail contents.
 */
export async function loadGuardrails(options) {
  const { cwd = process.cwd(), prompt = "" } = options || {};
  const bridgePath = path.join(cwd, "guardloop_bridge.py");

  try {
    // Execute the python script with the prompt as an argument.
    const output = execFileSync(bridgePath, [prompt], { encoding: "utf-8" });
    const result = JSON.parse(output);
    return result.guardrails.trim();
  } catch (error) {
    // If the script fails, log the error and return no guardrails.
    if (error instanceof Error) {
      console.error("Failed to execute GuardLoop bridge:", error.message);
    } else {
      console.error("Failed to execute GuardLoop bridge:", String(error));
    }
    return "";
  }
}
