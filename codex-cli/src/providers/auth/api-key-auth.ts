import type { AuthProvider } from "../types.js";

/**
 * Simple API key authentication provider
 */
export class ApiKeyAuthProvider implements AuthProvider {
  constructor(
    private apiKey: string | undefined,
    private providerName: string,
  ) {}

  async getAuthHeader(): Promise<string> {
    if (!this.apiKey) {
      throw new Error(`No API key found for ${this.providerName}`);
    }
    return `Bearer ${this.apiKey}`;
  }

  async validate(): Promise<void> {
    if (!this.apiKey || this.apiKey.trim() === "") {
      throw new Error(
        `No API key configured for ${this.providerName}. ` +
        `Please set the appropriate environment variable or update your config.`,
      );
    }
  }
}