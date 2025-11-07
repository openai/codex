// Anthropic Claude Integration
import Anthropic from '@anthropic-ai/sdk'
import type { AIMessage } from './types'

export async function createAnthropicClient(apiKey: string) {
  if (!apiKey) {
    throw new Error('Anthropic API key is required')
  }
  
  return new Anthropic({
    apiKey
  })
}

export async function* chatWithClaude(
  client: Anthropic,
  messages: AIMessage[],
  model: 'claude-4.5-sonnet' | 'claude-4.5-haiku' | 'claude-4.1-opus' = 'claude-4.5-sonnet',
  options?: {
    temperature?: number
    maxTokens?: number
    systemPrompt?: string
  }
) {
  const systemPrompt = options?.systemPrompt || 'You are a helpful AI coding assistant.'
  
  // Claude requires system prompt separate from messages
  const formattedMessages = messages.map(msg => ({
    role: msg.role as 'user' | 'assistant',
    content: msg.content
  }))
  
  const stream = await client.messages.create({
    model,
    max_tokens: options?.maxTokens ?? 4096,
    temperature: options?.temperature ?? 0.7,
    system: systemPrompt,
    messages: formattedMessages,
    stream: true
  })
  
  for await (const event of stream) {
    if (event.type === 'content_block_delta' && event.delta.type === 'text_delta') {
      yield event.delta.text
    }
  }
}

export async function chatWithClaudeNonStreaming(
  client: Anthropic,
  messages: AIMessage[],
  model: 'claude-4.5-sonnet' | 'claude-4.5-haiku' | 'claude-4.1-opus' = 'claude-4.5-sonnet',
  options?: {
    temperature?: number
    maxTokens?: number
    systemPrompt?: string
  }
): Promise<{ content: string; tokens: number }> {
  const systemPrompt = options?.systemPrompt || 'You are a helpful AI coding assistant.'
  
  const formattedMessages = messages.map(msg => ({
    role: msg.role as 'user' | 'assistant',
    content: msg.content
  }))
  
  const message = await client.messages.create({
    model,
    max_tokens: options?.maxTokens ?? 4096,
    temperature: options?.temperature ?? 0.7,
    system: systemPrompt,
    messages: formattedMessages,
    stream: false
  })
  
  const content = message.content
    .filter(block => block.type === 'text')
    .map(block => ('text' in block ? block.text : ''))
    .join('')
  
  return {
    content,
    tokens: message.usage.input_tokens + message.usage.output_tokens
  }
}

// Available Claude models
export const CLAUDE_MODELS = {
    'claude-4.1-opus': 'claude-4.1-opus',
    'claude-4.5-sonnet': 'claude-4.5-sonnet',
    'claude-4.5-haiku': 'claude-4.5-haiku',
} as const satisfies Record<string, string>

export type ClaudeModel = keyof typeof CLAUDE_MODELS

