/**
 * Keychain utilities for secure storage of sensitive information.
 *
 * This module provides functions to securely store and retrieve API keys
 * using the system's native keychain (macOS Keychain, Windows Credential Vault,
 * or Linux Secret Service API).
 */

import keytar from "keytar";
import { log } from "./agent/log.js";

// Service name for keychain entries
const SERVICE_NAME = "codex-cli";

// Account name format for provider API keys
const getAccountName = (provider: string): string => `${provider}-api-key`;

// Create a logger object with debug and error methods
const logger = {
  debug: (message: string, error?: unknown): void => {
    if (typeof log === "function") {
      log(`DEBUG: ${message} ${error ? JSON.stringify(error) : ""}`);
    }
  },
  error: (message: string, error?: unknown): void => {
    if (typeof log === "function") {
      log(`ERROR: ${message} ${error ? JSON.stringify(error) : ""}`);
    }
  },
};

/**
 * Check if the keychain is available on the current system.
 *
 * @returns Promise resolving to true if keychain is available, false otherwise
 */
export async function isKeychainAvailable(): Promise<boolean> {
  try {
    // Attempt a simple operation to check if keychain is available
    await keytar.findCredentials("codex-cli-test");
    return true;
  } catch (error) {
    logger.debug("Keychain is not available:", error);
    return false;
  }
}

/**
 * Store an API key in the system keychain.
 *
 * @param provider The provider name (e.g., "openai")
 * @param apiKey The API key to store
 * @returns Promise resolving to true if successful, false otherwise
 */
export async function storeApiKey(
  provider: string,
  apiKey: string,
): Promise<boolean> {
  try {
    const accountName = getAccountName(provider);
    await keytar.setPassword(SERVICE_NAME, accountName, apiKey);
    return true;
  } catch (error) {
    logger.error("Failed to store API key in keychain:", error);
    return false;
  }
}

/**
 * Retrieve an API key from the system keychain.
 *
 * @param provider The provider name (e.g., "openai")
 * @returns Promise resolving to the API key if found, null otherwise
 */
export async function getApiKey(provider: string): Promise<string | null> {
  try {
    const accountName = getAccountName(provider);
    const apiKey = await keytar.getPassword(SERVICE_NAME, accountName);
    return apiKey;
  } catch (error) {
    logger.error("Failed to retrieve API key from keychain:", error);
    return null;
  }
}

/**
 * Delete an API key from the system keychain.
 *
 * @param provider The provider name (e.g., "openai")
 * @returns Promise resolving to true if successful, false otherwise
 */
export async function deleteApiKey(provider: string): Promise<boolean> {
  try {
    const accountName = getAccountName(provider);
    const result = await keytar.deletePassword(SERVICE_NAME, accountName);
    return result;
  } catch (error) {
    logger.error("Failed to delete API key from keychain:", error);
    return false;
  }
}
