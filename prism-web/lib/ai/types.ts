// Shared AI types

export type AIProvider = 'openai' | 'anthropic'

// OpenAI Models
export type OpenAIModel =
  | 'gpt-5-pro'
  | 'gpt-5-me'
  | 'gpt-5-mini'
  // Legacy models
  | 'gpt-4-turbo-preview'
  | 'gpt-4'
  | 'gpt-3.5-turbo'

// Anthropic Models
export type ClaudeModel =
  | 'claude-4.5-sonnet'
  | 'claude-4.5-haiku'
  | 'claude-4.1-opus'
  // Legacy models
  | 'claude-3-opus-20240229'
  | 'claude-3-sonnet-20240229'
  | 'claude-3-haiku-20240307'

// All supported AI models
export type AIModel = OpenAIModel | ClaudeModel

export interface AIMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp?: Date
}

export interface AISession {
  id: string
  userId: string
  repositoryId?: string
  provider: AIProvider
  model: AIModel
  messages: AIMessage[]
  totalTokens: number
  createdAt: Date
  updatedAt: Date
}

export interface ChatOptions {
  temperature?: number
  maxTokens?: number
  systemPrompt?: string
}

export interface ChatResponse {
  content: string
  tokens: number
  model: string
  provider: AIProvider
}

export interface StreamChunk {
  content: string
  done: boolean
}

// Type guards
export function isOpenAIModel(model: string): model is OpenAIModel {
  return [
    'gpt-5-pro',
    'gpt-5-me',
    'gpt-5-mini',
    'gpt-4-turbo-preview',
    'gpt-4',
    'gpt-3.5-turbo'
  ].includes(model)
}

export function isClaudeModel(model: string): model is ClaudeModel {
  return [
    'claude-4.5-sonnet',
    'claude-4.5-haiku',
    'claude-4.1-opus',
    'claude-3-opus-20240229',
    'claude-3-sonnet-20240229',
    'claude-3-haiku-20240307'
  ].includes(model)
}

export function getProviderFromModel(model: AIModel): AIProvider {
  if (isOpenAIModel(model)) return 'openai'
  if (isClaudeModel(model)) return 'anthropic'
  throw new Error(`Unknown model: ${model}`)
}
