// OpenAI Integration
import OpenAI from 'openai'
import type { AIMessage } from './types'

export async function createOpenAIClient(apiKey: string) {
  if (!apiKey) {
    throw new Error('OpenAI API key is required')
  }
  
  return new OpenAI({
    apiKey,
    dangerouslyAllowBrowser: false // Server-side only
  })
}

export async function* chatWithOpenAI(
  client: OpenAI,
  messages: AIMessage[],
  model: 'gpt-5-pro' | 'gpt-5-me' | 'gpt-5-mini' | string = 'gpt-5-pro',
  options?: {
    temperature?: number
    maxTokens?: number
    systemPrompt?: string
  }
) {
  const systemPrompt = options?.systemPrompt || 'You are a helpful AI coding assistant.'
  
  const formattedMessages: OpenAI.ChatCompletionMessageParam[] = [
    { role: 'system', content: systemPrompt },
    ...messages.map(msg => ({
      role: msg.role as 'user' | 'assistant',
      content: msg.content
    }))
  ]
  
  const stream = await client.chat.completions.create({
    model,
    messages: formattedMessages,
    temperature: options?.temperature ?? 0.7,
    max_tokens: options?.maxTokens,
    stream: true
  })
  
  for await (const chunk of stream) {
    const content = chunk.choices[0]?.delta?.content
    if (content) {
      yield content
    }
  }
}

export async function chatWithOpenAINonStreaming(
  client: OpenAI,
  messages: AIMessage[],
  model: 'gpt-5-pro' | 'gpt-5-me' | 'gpt-5-mini' | string = 'gpt-5-pro',
  options?: {
    temperature?: number
    maxTokens?: number
    systemPrompt?: string
  }
): Promise<{ content: string; tokens: number }> {
  const systemPrompt = options?.systemPrompt || 'You are a helpful AI coding assistant.'
  
  const formattedMessages: OpenAI.ChatCompletionMessageParam[] = [
    { role: 'system', content: systemPrompt },
    ...messages.map(msg => ({
      role: msg.role as 'user' | 'assistant',
      content: msg.content
    }))
  ]
  
  const completion = await client.chat.completions.create({
    model,
    messages: formattedMessages,
    temperature: options?.temperature ?? 0.7,
    max_tokens: options?.maxTokens,
    stream: false
  })
  
  return {
    content: completion.choices[0]?.message?.content || '',
    tokens: completion.usage?.total_tokens || 0
  }
}

// Available OpenAI models
export const OPENAI_MODELS = {
  'gpt-5-codex': 'gpt-5-pro',
  'gpt-5-high': 'gpt-5-me',
  'gpt-5': 'gpt-5-mini',
} as const

export type OpenAIModel = keyof typeof OPENAI_MODELS

