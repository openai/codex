import type { GoogleAuthProvider } from "../auth/google-auth.js";
import type { ProviderConfig, AuthProvider } from "../types.js";
import type OpenAI from "openai";

import { BaseAdapter } from "./base-adapter.js";
import { log } from "../../utils/logger/log.js";
import { Stream } from "openai/streaming.mjs";

// Type definitions
interface RequestOptions {
  path?: string;
  url?: string;
  body?: string;
  headers?: Record<string, string>;
  [key: string]: unknown;
}

interface ChatMessage {
  role: string;
  content: string;
}

interface VertexContent {
  role: string;
  parts: Array<{ text: string }>;
}

/**
 * Adapter for Google Vertex AI
 * Uses a custom OpenAI client that intercepts requests to handle Vertex AI's API format
 */
export class VertexAdapter extends BaseAdapter {
  private googleAuth: GoogleAuthProvider;
  private modelMapping: Record<string, string> = {
    "gpt-4": "gemini-1.5-pro-002",
    "gpt-4-turbo": "gemini-1.5-pro-002",
    "gpt-3.5-turbo": "gemini-1.5-flash-002",
    "gemini-pro": "gemini-1.5-pro-002",
    "gemini-flash": "gemini-1.5-flash-002",
  };

  constructor(config: ProviderConfig, authProvider: AuthProvider) {
    super(config, authProvider);
    // We know this is always GoogleAuthProvider based on our registry
    this.googleAuth = authProvider as GoogleAuthProvider;
  }

  override async createClient(): Promise<OpenAI> {
    await this.authProvider.validate();
    
    // Create a custom client that intercepts requests
    const client = await super.createClient();
    
    // Store the original request method
    const originalRequest = (client as unknown as { request: (options: RequestOptions) => Promise<unknown> }).request.bind(client);
    
    // Override the request method
    (client as unknown as { request: (options: RequestOptions) => Promise<unknown> }).request = async (options: RequestOptions) => {
      // Intercept chat completions requests
      if (options.path?.includes("/chat/completions")) {
        return this.handleChatCompletions(options, originalRequest);
      }
      
      // Pass through other requests
      return originalRequest(options);
    };
    
    return client;
  }

  override async getBaseURL(): Promise<string> {
    const projectId = await this.getProjectId();
    const location = this.getLocation();
    
    return `https://${location}-aiplatform.googleapis.com/v1/projects/${projectId}/locations/${location}/publishers/google/models`;
  }

  private async handleChatCompletions(
    options: RequestOptions, 
    originalRequest: (options: RequestOptions) => Promise<unknown>
  ): Promise<unknown> {
    try {
      // Get fresh auth token
      const authHeader = await this.authProvider.getAuthHeader();
      options.headers = {
        ...options.headers,
        Authorization: authHeader,
        "Content-Type": "application/json",
      };

      // Parse the request body
      const body = JSON.parse(options.body || "{}") as { 
        model?: string; 
        messages?: Array<ChatMessage>; 
        temperature?: number;
        top_p?: number;
        max_tokens?: number;
      };
      const model = this.mapModelName(body.model || "gemini-1.5-flash-002");
      
      // Update URL for Vertex AI
      const baseURL = await this.getBaseURL();
      options.url = `${baseURL}/${model}:streamGenerateContent`;
      options.path = undefined;
      
      // Transform to Vertex AI format
      options.body = JSON.stringify({
        contents: this.transformMessages(body.messages || []),
        generationConfig: {
          temperature: body.temperature,
          topP: body.top_p,
          maxOutputTokens: body.max_tokens || 4096,
        },
        safetySettings: this.getSafetySettings(),
      });

      // Make the request
      const response = await originalRequest(options);
      
      // Transform the response back to OpenAI format
      return this.transformResponse(response, model);
    } catch (error) {
      log(`Vertex AI request failed: ${error}`);
      throw error;
    }
  }

  private transformMessages(messages: Array<ChatMessage>): Array<VertexContent> {
    const contents: Array<VertexContent> = [];
    let systemPrompt = "";
    
    for (const msg of messages) {
      if (msg.role === "system") {
        systemPrompt = msg.content;
        continue;
      }
      
      const parts = [];
      
      // Add system prompt to first user message
      if (msg.role === "user" && systemPrompt && contents.length === 0) {
        parts.push({ text: `${systemPrompt}\n\n${msg.content}` });
        systemPrompt = ""; // Clear it so we don't add it again
      } else {
        parts.push({ text: msg.content });
      }
      
      contents.push({
        role: msg.role === "assistant" ? "model" : "user",
        parts,
      });
    }
    
    return contents;
  }

  private transformResponse(vertexResponse: unknown, model: string): unknown {
    // Handle streaming
    if (vertexResponse instanceof Stream) {
      // TODO: Implement streaming transformation
      return vertexResponse;
    }

    // Transform non-streaming response
    const response = vertexResponse as {
      candidates?: Array<{
        content?: { parts?: Array<{ text?: string }> };
        finishReason?: string;
      }>;
      usageMetadata?: {
        promptTokenCount?: number;
        candidatesTokenCount?: number;
        totalTokenCount?: number;
      };
    };
    
    if (response.candidates?.[0]) {
      const candidate = response.candidates[0];
      const content = candidate.content?.parts?.[0]?.text || "";
      
      return {
        id: `vertex-${Date.now()}`,
        object: "chat.completion",
        created: Math.floor(Date.now() / 1000),
        model,
        choices: [{
          index: 0,
          message: {
            role: "assistant",
            content,
          },
          finish_reason: this.mapFinishReason(candidate.finishReason),
        }],
        usage: {
          prompt_tokens: response.usageMetadata?.promptTokenCount || 0,
          completion_tokens: response.usageMetadata?.candidatesTokenCount || 0,
          total_tokens: response.usageMetadata?.totalTokenCount || 0,
        },
      };
    }

    return vertexResponse;
  }

  private mapFinishReason(vertexReason?: string): string {
    const mapping: Record<string, string> = {
      "STOP": "stop",
      "MAX_TOKENS": "length",
      "SAFETY": "content_filter",
      "RECITATION": "content_filter",
    };
    return mapping[vertexReason || ""] || "stop";
  }

  private getSafetySettings() {
    return [
      { category: "HARM_CATEGORY_HATE_SPEECH", threshold: "BLOCK_ONLY_HIGH" },
      { category: "HARM_CATEGORY_DANGEROUS_CONTENT", threshold: "BLOCK_ONLY_HIGH" },
      { category: "HARM_CATEGORY_HARASSMENT", threshold: "BLOCK_ONLY_HIGH" },
      { category: "HARM_CATEGORY_SEXUALLY_EXPLICIT", threshold: "BLOCK_ONLY_HIGH" },
    ];
  }

  private async getProjectId(): Promise<string> {
    // Check environment variables
    const envProjectId = process.env["VERTEX_PROJECT_ID"] || 
                        process.env["GOOGLE_CLOUD_PROJECT"] || 
                        process.env["GCLOUD_PROJECT"];
    
    if (envProjectId) {
      return envProjectId;
    }

    // Try to get from auth provider
    const projectId = await this.googleAuth.getProjectId();
    if (projectId) {
      return projectId;
    }

    throw new Error(
      "No Google Cloud project ID found. Please set one of these environment variables:\n" +
      "- VERTEX_PROJECT_ID\n" +
      "- GOOGLE_CLOUD_PROJECT\n" +
      "- GCLOUD_PROJECT",
    );
  }

  private getLocation(): string {
    return process.env["VERTEX_LOCATION"] || 
           process.env["VERTEX_AI_LOCATION"] || 
           "us-central1";
  }

  mapModelName(model: string): string {
    return this.modelMapping[model] || model;
  }
}