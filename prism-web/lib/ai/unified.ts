// Unified AI Interface for Multi-LLM support
import { createOpenAIClient, chatWithOpenAI, chatWithOpenAINonStreaming } from './openai'
import { createAnthropicClient, chatWithClaude, chatWithClaudeNonStreaming } from './anthropic'
import type { AIProvider, AIMessage, ChatOptions, ChatResponse } from './types'

export async function* chat(
  provider: AIProvider,
  apiKey: string,
  messages: AIMessage[],
  model: string,
  options?: ChatOptions
): AsyncGenerator<string> {
  switch (provider) {
    case 'openai': {
      const client = await createOpenAIClient(apiKey)
      yield* chatWithOpenAI(client, messages, model, options)
      break
    }
    case 'anthropic': {
      const client = await createAnthropicClient(apiKey)
      yield* chatWithClaude(client, messages, model, options)
      break
    }
    default:
      throw new Error(`Unsupported provider: ${provider}`)
  }
}

export async function chatNonStreaming(
  provider: AIProvider,
  apiKey: string,
  messages: AIMessage[],
  model: string,
  options?: ChatOptions
): Promise<ChatResponse> {
  let result: { content: string; tokens: number }
  
  switch (provider) {
    case 'openai': {
      const client = await createOpenAIClient(apiKey)
      result = await chatWithOpenAINonStreaming(client, messages, model, options)
      break
    }
    case 'anthropic': {
      const client = await createAnthropicClient(apiKey)
      result = await chatWithClaudeNonStreaming(client, messages, model, options)
      break
    }
    default:
      throw new Error(`Unsupported provider: ${provider}`)
  }
  
  return {
    ...result,
    model,
    provider
  }
}

// Model suggestions based on task type
export function suggestModel(taskType: 'code' | 'chat' | 'analysis' | 'review'): {
  provider: AIProvider
  model: string
  reason: string
} {
  switch (taskType) {
    case 'code':
      return {
        provider: 'openai',
        model: 'gpt-4-turbo-preview',
        reason: 'Best for code generation and completion'
      }
    case 'chat':
      return {
        provider: 'anthropic',
        model: 'claude-3-sonnet-20240229',
        reason: 'Great balance of speed and quality for conversation'
      }
    case 'analysis':
      return {
        provider: 'anthropic',
        model: 'claude-3-opus-20240229',
        reason: 'Superior reasoning for complex analysis'
      }
    case 'review':
      return {
        provider: 'openai',
        model: 'gpt-4',
        reason: 'Excellent code review capabilities'
      }
  }
}

// Estimate cost per 1K tokens (2025蟷ｴ11譛域怙譁ｰ)
export function estimateCost(provider: AIProvider, model: string): {
  input: number // $ per 1K tokens
  output: number // $ per 1K tokens
} {
  // Pricing as of Nov 2025
  const pricing: Record<AIProvider, Record<string, { input: number; output: number }>> = {
    openai: {
      'gpt-5-pro': { input: 0.015, output: 0.060 },
      'gpt-5-me': { input: 0.010, output: 0.030 },
      'gpt-5-mini': { input: 0.0005, output: 0.002 },
      // Legacy models
      'gpt-4-turbo-preview': { input: 0.01, output: 0.03 },
      'gpt-4': { input: 0.03, output: 0.06 }
    },
    anthropic: {
      'claude-4.5-sonnet': { input: 0.003, output: 0.015 },
      'claude-4.5-haiku': { input: 0.0004, output: 0.002 },
      'claude-4.1-opus': { input: 0.015, output: 0.075 },
      // Legacy models
      'claude-3-opus-20240229': { input: 0.015, output: 0.075 },
      'claude-3-sonnet-20240229': { input: 0.003, output: 0.015 },
      'claude-3-haiku-20240307': { input: 0.00025, output: 0.00125 }
    }
  }
  
  return pricing[provider]?.[model] || { input: 0, output: 0 }
}

