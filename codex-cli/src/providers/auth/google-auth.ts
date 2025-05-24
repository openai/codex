import type { AuthProvider } from "../types.js";

import { log } from "../../utils/logger/log.js";
import { GoogleAuth } from "google-auth-library";

/**
 * Google OAuth authentication provider using Application Default Credentials
 */
export class GoogleAuthProvider implements AuthProvider {
  private authClient: GoogleAuth;
  private cachedToken: { token: string; expiry: number } | null = null;

  constructor() {
    this.authClient = new GoogleAuth({
      scopes: ["https://www.googleapis.com/auth/cloud-platform"],
    });
  }

  async getAuthHeader(): Promise<string> {
    // Check cached token
    if (this.cachedToken && this.cachedToken.expiry > Date.now()) {
      return `Bearer ${this.cachedToken.token}`;
    }

    // Get new token
    const client = await this.authClient.getClient();
    const tokenResponse = await client.getAccessToken();

    if (!tokenResponse.token) {
      throw new Error("Failed to get access token from Google Auth");
    }

    // Cache for 55 minutes (tokens typically last 1 hour)
    this.cachedToken = {
      token: tokenResponse.token,
      expiry: Date.now() + 55 * 60 * 1000,
    };

    return `Bearer ${tokenResponse.token}`;
  }

  async validate(): Promise<void> {
    try {
      await this.authClient.getClient();
    } catch (error) {
      log(`Google Auth validation failed: ${error}`);
      throw new Error(
        "Failed to authenticate with Google Cloud. Please ensure you have valid credentials:\n" +
          "- Run 'gcloud auth application-default login' to use your user credentials\n" +
          "- Or set GOOGLE_APPLICATION_CREDENTIALS to point to a service account key file\n" +
          "- Or run this on a Google Cloud compute resource with appropriate IAM roles",
      );
    }
  }

  async getProjectId(): Promise<string | null> {
    try {
      return await this.authClient.getProjectId();
    } catch {
      return null;
    }
  }
}
