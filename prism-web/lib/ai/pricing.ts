// AI Model Pricing Information (2025年11月最新)

export interface ModelPricing {
  provider: 'openai' | 'anthropic'
  model: string
  displayName: string
  inputPricePer1K: number  // USD per 1K tokens
  outputPricePer1K: number // USD per 1K tokens
  contextWindow: number    // Max tokens
  description: string
  recommendedFor: string[]
}

export const MODEL_PRICING: Record<string, ModelPricing> = {
  // OpenAI GPT-5 Series
  'gpt-5-pro': {
    provider: 'openai',
    model: 'gpt-5-pro',
    displayName: 'GPT-5 Pro (Codex)',
    inputPricePer1K: 0.015,
    outputPricePer1K: 0.060,
    contextWindow: 128000,
    description: '最高品質のコード生成と複雑な推論',
    recommendedFor: ['複雑なリファクタリング', 'アーキテクチャ設計', '最高品質コード生成']
  },
  'gpt-5-me': {
    provider: 'openai',
    model: 'gpt-5-me',
    displayName: 'GPT-5 Medium',
    inputPricePer1K: 0.010,
    outputPricePer1K: 0.030,
    contextWindow: 128000,
    description: 'バランスの取れた性能とコスト',
    recommendedFor: ['一般的なコーディング', 'コードレビュー', 'バグ修正']
  },
  'gpt-5-mini': {
    provider: 'openai',
    model: 'gpt-5-mini',
    displayName: 'GPT-5 Mini',
    inputPricePer1K: 0.0005,
    outputPricePer1K: 0.002,
    contextWindow: 128000,
    description: '高速・低コストの軽量モデル',
    recommendedFor: ['簡単なコード補完', '構文チェック', '高速処理']
  },
  
  // Anthropic Claude 4 Series
  'claude-4.5-sonnet': {
    provider: 'anthropic',
    model: 'claude-4.5-sonnet',
    displayName: 'Claude 4.5 Sonnet',
    inputPricePer1K: 0.003,
    outputPricePer1K: 0.015,
    contextWindow: 200000,
    description: '2025年最新の標準モデル、優れたバランス',
    recommendedFor: ['コードレビュー', '詳細な説明', 'ペアプログラミング']
  },
  'claude-4.5-haiku': {
    provider: 'anthropic',
    model: 'claude-4.5-haiku',
    displayName: 'Claude 4.5 Haiku',
    inputPricePer1K: 0.0004,
    outputPricePer1K: 0.002,
    contextWindow: 200000,
    description: '超高速・超低コストモデル',
    recommendedFor: ['リアルタイム補完', 'クイック質問', '大量処理']
  },
  'claude-4.1-opus': {
    provider: 'anthropic',
    model: 'claude-4.1-opus',
    displayName: 'Claude 4.1 Opus',
    inputPricePer1K: 0.015,
    outputPricePer1K: 0.075,
    contextWindow: 200000,
    description: '最高性能の推論と分析',
    recommendedFor: ['複雑な問題解決', '深い分析', 'セキュリティ監査']
  }
}

// Calculate estimated cost
export function calculateCost(
  model: string,
  inputTokens: number,
  outputTokens: number
): number {
  const pricing = MODEL_PRICING[model]
  if (!pricing) return 0
  
  const inputCost = (inputTokens / 1000) * pricing.inputPricePer1K
  const outputCost = (outputTokens / 1000) * pricing.outputPricePer1K
  
  return inputCost + outputCost
}

// Get cost comparison for 100K tokens (typical usage)
export function getCostComparison() {
  const testTokens = 100000
  const results = Object.entries(MODEL_PRICING).map(([key, pricing]) => ({
    model: key,
    displayName: pricing.displayName,
    cost: calculateCost(key, testTokens / 2, testTokens / 2),
    provider: pricing.provider
  }))
  
  return results.sort((a, b) => a.cost - b.cost)
}

// Recommend model based on task type and budget
export function recommendModel(
  taskType: 'code-generation' | 'code-review' | 'chat' | 'analysis',
  budget: 'low' | 'medium' | 'high'
): string {
  const recommendations: Record<string, Record<string, string>> = {
    'code-generation': {
      low: 'gpt-5-mini',
      medium: 'gpt-5-me',
      high: 'gpt-5-pro'
    },
    'code-review': {
      low: 'claude-4.5-haiku',
      medium: 'claude-4.5-sonnet',
      high: 'claude-4.1-opus'
    },
    chat: {
      low: 'claude-4.5-haiku',
      medium: 'claude-4.5-sonnet',
      high: 'claude-4.5-sonnet'
    },
    analysis: {
      low: 'claude-4.5-sonnet',
      medium: 'claude-4.1-opus',
      high: 'claude-4.1-opus'
    }
  }
  
  return recommendations[taskType][budget]
}

// Format price for display
export function formatPrice(price: number): string {
  if (price < 0.01) return `$${(price * 1000).toFixed(2)}/M tokens`
  return `$${price.toFixed(3)}/1K tokens`
}

