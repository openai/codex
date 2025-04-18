import chalk from "chalk";
import keytar from "keytar";

// Service and account names for secure key storage.
const SERVICE_NAME = "@openai/codex";
const ACCOUNT_NAME = "OPENAI_API_KEY";

/**
 * @returns The OpenAI API key, or empty string if none is found.
 */
export async function getApiKey(): Promise<string> {
  try {
    const storedKey = await keytar.getPassword(SERVICE_NAME, ACCOUNT_NAME);
    if (storedKey) {
      return storedKey;
    }
  } catch {
    // ignore
  }
  return "";
}

/**
 * Persist the provided API key in secure storage.
 *
 * @param apiKey The OpenAI API key to store.
 */
export async function setApiKey(apiKey: string): Promise<void> {
  try {
    await keytar.setPassword(SERVICE_NAME, ACCOUNT_NAME, apiKey);
  } catch {
    // eslint-disable-next-line no-console
    console.error(
      chalk.yellow(
        "Failed to securely store the API key. You can set it manually in your .bashrc or .zshrc file. However, this is not recommended for security reasons as the key will be in plain text.",
      ),
    );
  }
}
