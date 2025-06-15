import type { AppConfig } from "./config.js";
import type { ClientOptions } from "openai";
import type * as Core from "openai/core";

import {
  getBaseUrl,
  getApiKey,
  AZURE_OPENAI_API_VERSION,
  OPENAI_TIMEOUT_MS,
  OPENAI_ORGANIZATION,
  OPENAI_PROJECT,
} from "./config.js";
import OpenAI, { AzureOpenAI } from "openai";
import * as Errors from "openai/error";

type OpenAIClientConfig = {
  provider: string;
};

/**
 * Creates an OpenAI client instance based on the provided configuration.
 * Handles both standard OpenAI and Azure OpenAI configurations.
 *
 * @param config The configuration containing provider information
 * @returns An instance of either OpenAI or AzureOpenAI client
 */
export function createOpenAIClient(
  config: OpenAIClientConfig | AppConfig,
): OpenAI | AzureOpenAI {
  const headers: Record<string, string> = {};
  if (OPENAI_ORGANIZATION) {
    headers["OpenAI-Organization"] = OPENAI_ORGANIZATION;
  }
  if (OPENAI_PROJECT) {
    headers["OpenAI-Project"] = OPENAI_PROJECT;
  }

  if (config.provider?.toLowerCase() === "azure") {
    return new AzureOpenAI({
      apiKey: getApiKey(config.provider),
      baseURL: getBaseUrl(config.provider),
      apiVersion: AZURE_OPENAI_API_VERSION,
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: headers,
    });
  }

  if (config.provider?.toLowerCase() === "githubcopilot") {
    return new GithubCopilotClient({
      apiKey: getApiKey(config.provider),
      baseURL: getBaseUrl(config.provider),
      timeout: OPENAI_TIMEOUT_MS,
      defaultHeaders: headers,
    });
  }

  return new OpenAI({
    apiKey: getApiKey(config.provider),
    baseURL: getBaseUrl(config.provider),
    timeout: OPENAI_TIMEOUT_MS,
    defaultHeaders: headers,
  });
}

export class GithubCopilotClient extends OpenAI {
  private copilotToken: string | null = null;
  private copilotTokenExpiration = new Date();
  private githubAPIKey: string;

  constructor(opts: ClientOptions = {}) {
    super(opts);
    if (!opts.apiKey) {
      throw new Errors.OpenAIError("missing github copilot token");
    }
    this.githubAPIKey = opts.apiKey;
  }

  private async _getGithubCopilotToken(): Promise<string | undefined> {
    if (
      this.copilotToken &&
      this.copilotTokenExpiration.getTime() > Date.now()
    ) {
      return this.copilotToken;
    }
    const resp = await fetch(
      "https://api.github.com/copilot_internal/v2/token",
      {
        method: "GET",
        headers: GithubCopilotClient._mergeGithubHeaders({
          "Authorization": `bearer ${this.githubAPIKey}`,
          "Accept": "application/json",
          "Content-Type": "application/json",
        }),
      },
    );
    if (!resp.ok) {
      const text = await resp.text();
      throw new Error("unable to get github copilot auth token: " + text);
    }
    const text = await resp.text();
    const { token, refresh_in } = JSON.parse(text);
    if (typeof token !== "string" || typeof refresh_in !== "number") {
      throw new Errors.OpenAIError(
        `unexpected response from copilot auth: ${text}`,
      );
    }
    this.copilotToken = token;
    this.copilotTokenExpiration = new Date(Date.now() + refresh_in * 1000);
    return token;
  }

  protected override authHeaders(
    _opts: Core.FinalRequestOptions,
  ): Core.Headers {
    return {};
  }

  protected override async prepareOptions(
    opts: Core.FinalRequestOptions<unknown>,
  ): Promise<void> {
    const token = await this._getGithubCopilotToken();
    opts.headers ??= {};
    if (token) {
      opts.headers["Authorization"] = `Bearer ${token}`;
      opts.headers = GithubCopilotClient._mergeGithubHeaders(opts.headers);
    } else {
      throw new Errors.OpenAIError("Unable to handle auth");
    }
    return super.prepareOptions(opts);
  }

  static async getLoginURL(): Promise<{
    device_code: string;
    user_code: string;
    verification_uri: string;
  }> {
    const resp = await fetch("https://github.com/login/device/code", {
      method: "POST",
      headers: this._mergeGithubHeaders({
        "Content-Type": "application/json",
        "accept": "application/json",
      }),
      body: JSON.stringify({
        client_id: "Iv1.b507a08c87ecfe98",
        scope: "read:user",
      }),
    });
    if (!resp.ok) {
      const text = await resp.text();
      throw new Errors.OpenAIError("Unable to get login device code: " + text);
    }
    return resp.json();
  }

  static async pollForAccessToken(deviceCode: string): Promise<string> {
    /*eslint no-await-in-loop: "off"*/
    const MAX_ATTEMPTS = 36;
    let lastErr: unknown = null;
    for (let i = 0; i < MAX_ATTEMPTS; ++i) {
      try {
        const resp = await fetch(
          "https://github.com/login/oauth/access_token",
          {
            method: "POST",
            headers: this._mergeGithubHeaders({
              "Content-Type": "application/json",
              "accept": "application/json",
            }),
            body: JSON.stringify({
              client_id: "Iv1.b507a08c87ecfe98",
              device_code: deviceCode,
              grant_type: "urn:ietf:params:oauth:grant-type:device_code",
            }),
          },
        );
        if (!resp.ok) {
          continue;
        }
        const info = await resp.json();
        if (info.access_token) {
          return info.access_token as string;
        } else if (info.error === "authorization_pending") {
          lastErr = null;
        } else {
          throw new Errors.OpenAIError(
            "unexpected response when polling for access token: " +
              JSON.stringify(info),
          );
        }
      } catch (err) {
        lastErr = err;
      }
      await new Promise((resolve) => setTimeout(resolve, 5_000));
    }
    throw new Errors.OpenAIError(
      "timed out waiting for access token",
      lastErr != null ? { cause: lastErr } : {},
    );
  }

  private static _mergeGithubHeaders<
    T extends Core.Headers | Record<string, string>,
  >(headers: T): T {
    const copy = { ...headers } as Record<string, string> & T;
    copy["User-Agent"] = "GithubCopilot/1.155.0";
    copy["editor-version"] = "vscode/1.85.1";
    copy["editor-plugin-version"] = "copilot/1.155.0";
    return copy as T;
  }
}
