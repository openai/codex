import { BaseAdapter } from "./base-adapter.js";
import {
  AZURE_OPENAI_API_VERSION,
  OPENAI_TIMEOUT_MS,
} from "../../utils/config.js";
import { AzureOpenAI } from "openai";

/**
 * Adapter for Azure OpenAI with its specific requirements
 */
export class AzureAdapter extends BaseAdapter {
  override async createClient(): Promise<AzureOpenAI> {
    await this.validateConfiguration();
    await this.authProvider.validate();

    const authHeader = await this.authProvider.getAuthHeader();
    const baseURL = await this.getBaseURL();
    const additionalHeaders =
      (await this.authProvider.getAdditionalHeaders?.()) || {};

    return new AzureOpenAI({
      apiKey: authHeader.replace("Bearer ", ""),
      baseURL,
      apiVersion: AZURE_OPENAI_API_VERSION,
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: {
        ...additionalHeaders,
      },
    });
  }

  protected override async validateConfiguration(): Promise<void> {
    // Azure requires a base URL to be set
    const envKey = `${this.config.id.toUpperCase()}_BASE_URL`;
    const baseURL = process.env[envKey] || this.config.baseURL;

    if (!baseURL) {
      throw new Error(
        `Azure OpenAI requires a base URL. Please set the ${envKey} environment variable ` +
          `to your Azure OpenAI endpoint (e.g., https://YOUR-RESOURCE.openai.azure.com/openai)`,
      );
    }
  }
}
