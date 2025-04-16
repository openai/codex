import clipboardy from "clipboardy";
import readline from "readline";

/**
 * Parse raw user input into a list of model IDs.
 * Splits on newlines, trims whitespace, and filters out empty lines.
 */
export function parseManualModels(input: string): Array<string> {
  return input
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/**
 * Interactive prompt for manual model listing in manual LLM mode.
 * Copies a prompt to the clipboard and reads user-pasted model list.
 */
export async function manualFetchModels(): Promise<Array<string>> {
  const prompt = "Please list available OpenAI models, one per line.";
  try {
    await clipboardy.write(prompt);
    // eslint-disable-next-line no-console
    console.log("Manual LLM mode: prompt copied to clipboard.");
    // eslint-disable-next-line no-console
    console.log(
      "Please paste the list of models below, one per line. Submit an empty line to finish."
    );
  } catch (err) {
    // If clipboard write fails, still proceed to prompt user
    // eslint-disable-next-line no-console
    console.warn(
      "Warning: failed to write prompt to clipboard. Please copy it manually:\n" + prompt
    );
  }
  const lines: Array<string> = [];
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  for await (const line of rl) {
    const trimmed = line.trim();
    if (trimmed === "") {
      rl.close();
      break;
    }
    lines.push(trimmed);
  }
  return lines;
}