// Import external dependencies first
import type { ClientOptions } from 'openai';
import OpenAI from 'openai';

// Then import internal dependencies after a blank line
import { log, isLoggingEnabled } from './agent/log.js';

// Default configuration for retry behavior
const DEFAULT_MAX_RETRIES = 10;
const DEFAULT_INITIAL_DELAY_MS = 1000;
const DEFAULT_MAX_DELAY_MS = 60000; // Cap at 60 seconds

export interface BackoffOptions {
  /** Maximum number of retry attempts */
  maxRetries?: number;
  /** Initial delay in milliseconds */
  initialDelayMs?: number;
  /** Maximum delay in milliseconds */
  maxDelayMs?: number;
}

/**
 * Wrapper for OpenAI API calls that implements exponential backoff on rate limit errors.
 * This function will retry the API call with increasing delays when rate limits are hit.
 * 
 * @param fn The OpenAI API function to call
 * @param args Arguments to pass to the function
 * @param options Backoff configuration options
 * @returns The result of the API call
 * @throws Rethrows any non-rate-limit errors or if max retries is exceeded
 */
export async function withBackoff<T, TArgs extends Array<unknown>>(
  fn: (...args: TArgs) => Promise<T>,
  args: TArgs = [] as unknown as TArgs,
  options: BackoffOptions = {}
): Promise<T> {
  const maxRetries = options.maxRetries ?? DEFAULT_MAX_RETRIES;
  const initialDelayMs = options.initialDelayMs ?? DEFAULT_INITIAL_DELAY_MS;
  const maxDelayMs = options.maxDelayMs ?? DEFAULT_MAX_DELAY_MS;
  
  let delayMs = initialDelayMs;
  
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      // eslint-disable-next-line no-await-in-loop
      return await fn(...args);
    } catch (error) {
      // Type assertion for error object
      const err = error as { 
        status?: number; 
        httpStatus?: number; 
        statusCode?: number;
        code?: string;
        type?: string;
        message?: string;
        headers?: Record<string, string>;
      };
      
      // Check if it's a rate limit error
      const status = err?.status ?? err?.httpStatus ?? err?.statusCode;
      const isRateLimit = 
        status === 429 || 
        err?.code === "rate_limit_exceeded" || 
        err?.type === "rate_limit_exceeded" ||
        /rate limit/i.test(err?.message ?? "");
      
      if (!isRateLimit || attempt >= maxRetries) {
        throw error;
      }
      
      // Try to parse retry-after header or extract time from error message
      let suggestedDelay: number | null = null;
      
      // Check for retry-after header (seconds)
      const retryAfter = err?.headers?.["retry-after"];
      if (retryAfter && !isNaN(parseInt(retryAfter, 10))) {
        suggestedDelay = parseInt(retryAfter, 10) * 1000;
      }
      
      // Parse suggested retry time from error message, e.g., "Please try again in 1.3s"
      if (!suggestedDelay) {
        const msg = err?.message ?? "";
        const match = /retry again in ([\d.]+)s/i.exec(msg);
        if (match && match[1]) {
          const suggested = parseFloat(match[1]) * 1000;
          if (!Number.isNaN(suggested)) {
            suggestedDelay = suggested;
          }
        }
      }
      
      // Use suggested delay or exponential backoff
      const actualDelay = suggestedDelay || delayMs;
      
      if (isLoggingEnabled()) {
        log(`OpenAI rate limit hit, retrying in ${Math.round(actualDelay)}ms (attempt ${attempt}/${maxRetries})`);
      }
      
      // Wait before retrying
      // eslint-disable-next-line no-await-in-loop
      await new Promise(resolve => setTimeout(resolve, actualDelay));
      
      // Calculate next backoff delay (exponential with jitter)
      delayMs = Math.min(delayMs * 2 * (0.9 + Math.random() * 0.2), maxDelayMs);
    }
  }
  
  throw new Error(`Exceeded maximum retry attempts (${maxRetries}) for OpenAI API call`);
}

/**
 * Creates an enhanced OpenAI client with built-in exponential backoff for rate limits.
 * This wraps the standard OpenAI client methods that make API calls with retry logic.
 * 
 * @param options Standard OpenAI client options plus optional backoff configuration
 * @returns An OpenAI client with enhanced retry capabilities
 */
export function createOpenAIClientWithBackoff(
  clientOptions: ClientOptions & { backoffOptions?: BackoffOptions }
): OpenAI {
  const { backoffOptions, ...openAIOptions } = clientOptions;
  const client = new OpenAI(openAIOptions);
  
  // Enhancement target methods that should have backoff
  const methodsToEnhance: Array<Array<string>> = [
    ['completions', 'create'],
    ['chat', 'completions', 'create'],
    ['responses', 'create'],
  ];
  
  // Apply backoff wrapper to each method
  for (const path of methodsToEnhance) {
    let target: unknown = client;
    let valid = true;
    
    // Navigate to the nested object where the method is
    for (let i = 0; i < path.length - 1; i++) {
      const key = path[i];
      if (!key) {
        valid = false;
        break;
      }
      
      target = (target as Record<string, unknown>)[key];
      if (!target) {
        valid = false;
        break;
      }
    }
    
    if (!valid) {
      continue;
    }
    
    const lastKey = path[path.length - 1];
    if (!lastKey) {
      continue;
    }
    
    // If we found the target object and it has the method, wrap it
    if (typeof (target as Record<string, unknown>)[lastKey] === 'function') {
      const originalMethod = (target as Record<string, unknown>)[lastKey] as (...args: Array<unknown>) => Promise<unknown>;
      
      // Replace with wrapped version
      (target as Record<string, unknown>)[lastKey] = async (...args: Array<unknown>) => {
        return withBackoff(originalMethod.bind(target as object), args, backoffOptions);
      };
    }
  }
  
  return client;
} 