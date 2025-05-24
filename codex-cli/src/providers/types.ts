/**
 * Core types and interfaces for the provider abstraction layer.
 * This enables support for different authentication methods and provider-specific requirements.
 */

import type OpenAI from "openai";

/**
 * Authentication types supported by providers
 */
export enum AuthType {
  /** Simple API key authentication */
  API_KEY = "api_key",
  /** OAuth2 bearer token authentication */
  OAUTH = "oauth",
  /** No authentication required */
  NONE = "none",
}

/**
 * Base provider configuration
 */
export interface ProviderConfig {
  /** Unique identifier for the provider */
  id: string;
  /** Display name for the provider */
  name: string;
  /** Base URL for the API (may be dynamic for some providers) */
  baseURL: string;
  /** Environment variable name for credentials */
  envKey: string;
  /** Type of authentication used */
  authType: AuthType;
  /** Optional custom configuration */
  customConfig?: Record<string, unknown>;
}

/**
 * Authentication provider interface
 */
export interface AuthProvider {
  /** Get the authentication header value */
  getAuthHeader(): Promise<string>;
  /** Validate that authentication is properly configured */
  validate(): Promise<void>;
  /** Get any additional headers needed for requests */
  getAdditionalHeaders?(): Promise<Record<string, string>>;
}

/**
 * Provider adapter interface for creating OpenAI-compatible clients
 */
export interface ProviderAdapter {
  /** The provider configuration */
  config: ProviderConfig;
  /** Create an OpenAI-compatible client */
  createClient(): Promise<OpenAI>;
  /** Get the effective base URL (may be dynamic) */
  getBaseURL(): Promise<string>;
  /** Transform model names if needed */
  mapModelName?(model: string): string;
}

/**
 * Factory for creating auth providers
 */
export type AuthProviderFactory = (config: ProviderConfig) => AuthProvider | Promise<AuthProvider>;

/**
 * Factory for creating provider adapters
 */
export type ProviderAdapterFactory = (
  config: ProviderConfig,
  authProvider: AuthProvider
) => ProviderAdapter;