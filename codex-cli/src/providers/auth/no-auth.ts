import type { AuthProvider } from "../types.js";

/**
 * No-op authentication provider for services that don't require auth
 */
export class NoAuthProvider implements AuthProvider {
  async getAuthHeader(): Promise<string> {
    return "Bearer dummy";
  }

  async validate(): Promise<void> {
    // No validation needed
  }
}
