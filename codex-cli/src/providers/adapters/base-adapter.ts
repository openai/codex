import type { ProviderAdapter, ProviderConfig, AuthProvider } from "../types.js";

import { OPENAI_TIMEOUT_MS } from "../../utils/config.js";
import OpenAI from "openai";

/**
 * Base adapter that handles common functionality for most providers
 */
export abstract class BaseAdapter implements ProviderAdapter {
  constructor(
    public config: ProviderConfig,
    protected authProvider: AuthProvider,
  ) {}

  async createClient(): Promise<OpenAI> {
    await this.validateConfiguration();
    await this.authProvider.validate();
    
    const authHeader = await this.authProvider.getAuthHeader();
    const baseURL = await this.getBaseURL();
    const additionalHeaders = await this.authProvider.getAdditionalHeaders?.() || {};

    return new OpenAI({
      apiKey: authHeader.replace("Bearer ", ""), // OpenAI SDK expects just the key
      baseURL,
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: {
        ...additionalHeaders,
      },
    });
  }

  async getBaseURL(): Promise<string> {
    // Check for environment variable override
    const envKey = `${this.config.id.toUpperCase()}_BASE_URL`;
    const baseURL = process.env[envKey] || this.config.baseURL;
    
    if (!baseURL) {
      throw new Error(
        `No base URL configured for ${this.config.name}. ` +
        `Please set the ${envKey} environment variable.`
      );
    }
    
    return baseURL;
  }

  /**
   * Validate provider-specific configuration
   * Override in subclasses for custom validation
   */
  protected async validateConfiguration(): Promise<void> {
    // Default implementation does nothing
  }
}