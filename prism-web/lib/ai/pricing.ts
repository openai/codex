// AI Model Pricing Information (2025蟷ｴ11譛域怙譁ｰ)

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
    description: '譛鬮伜刀雉ｪ縺ｮ繧ｳ繝ｼ繝臥函謌舌→隍・尅縺ｪ謗ｨ隲・,
    recommendedFor: ['隍・尅縺ｪ繝ｪ繝輔ぃ繧ｯ繧ｿ繝ｪ繝ｳ繧ｰ', '繧｢繝ｼ繧ｭ繝・け繝√Ε險ｭ險・, '譛鬮伜刀雉ｪ繧ｳ繝ｼ繝臥函謌・]
  },
  'gpt-5-me': {
    provider: 'openai',
    model: 'gpt-5-me',
    displayName: 'GPT-5 Medium',
    inputPricePer1K: 0.010,
    outputPricePer1K: 0.030,
    contextWindow: 128000,
    description: '繝舌Λ繝ｳ繧ｹ縺ｮ蜿悶ｌ縺滓ｧ閭ｽ縺ｨ繧ｳ繧ｹ繝・,
    recommendedFor: ['荳闊ｬ逧・↑繧ｳ繝ｼ繝・ぅ繝ｳ繧ｰ', '繧ｳ繝ｼ繝峨Ξ繝薙Η繝ｼ', '繝舌げ菫ｮ豁｣']
  },
  'gpt-5-mini': {
    provider: 'openai',
    model: 'gpt-5-mini',
    displayName: 'GPT-5 Mini',
    inputPricePer1K: 0.0005,
    outputPricePer1K: 0.002,
    contextWindow: 128000,
    description: '鬮倬溘・菴弱さ繧ｹ繝医・霆ｽ驥上Δ繝・Ν',
    recommendedFor: ['邁｡蜊倥↑繧ｳ繝ｼ繝芽｣懷ｮ・, '讒区枚繝√ぉ繝・け', '鬮倬溷・逅・]
  },
  
  // Anthropic Claude 4 Series
  'claude-4.5-sonnet': {
    provider: 'anthropic',
    model: 'claude-4.5-sonnet',
    displayName: 'Claude 4.5 Sonnet',
    inputPricePer1K: 0.003,
    outputPricePer1K: 0.015,
    contextWindow: 200000,
    description: '2025蟷ｴ譛譁ｰ縺ｮ讓呎ｺ悶Δ繝・Ν縲∝━繧後◆繝舌Λ繝ｳ繧ｹ',
    recommendedFor: ['繧ｳ繝ｼ繝峨Ξ繝薙Η繝ｼ', '隧ｳ邏ｰ縺ｪ隱ｬ譏・, '繝壹い繝励Ο繧ｰ繝ｩ繝溘Φ繧ｰ']
  },
  'claude-4.5-haiku': {
    provider: 'anthropic',
    model: 'claude-4.5-haiku',
    displayName: 'Claude 4.5 Haiku',
    inputPricePer1K: 0.0004,
    outputPricePer1K: 0.002,
    contextWindow: 200000,
    description: '雜・ｫ倬溘・雜・ｽ弱さ繧ｹ繝医Δ繝・Ν',
    recommendedFor: ['繝ｪ繧｢繝ｫ繧ｿ繧､繝陬懷ｮ・, '繧ｯ繧､繝・け雉ｪ蝠・, '螟ｧ驥丞・逅・]
  },
  'claude-4.1-opus': {
    provider: 'anthropic',
    model: 'claude-4.1-opus',
    displayName: 'Claude 4.1 Opus',
    inputPricePer1K: 0.015,
    outputPricePer1K: 0.075,
    contextWindow: 200000,
    description: '譛鬮俶ｧ閭ｽ縺ｮ謗ｨ隲悶→蛻・梵',
    recommendedFor: ['隍・尅縺ｪ蝠城｡瑚ｧ｣豎ｺ', '豺ｱ縺・・譫・, '繧ｻ繧ｭ繝･繝ｪ繝・ぅ逶｣譟ｻ']
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

