/**
 * Model pricing information for different providers and models.
 * Pricing is in USD per 1M tokens.
 */
export interface ModelPricing {
  inputPrice: number;  // Price per 1M input tokens
  outputPrice: number; // Price per 1M output tokens
  contextWindow: number; // Maximum context window size
}

/**
 * Mapping of OpenRouter model identifiers to their pricing information
 * Prices are in USD per 1M tokens
 */
export const modelPricing: Record<string, ModelPricing> = {
  // Default fallback pricing
  "default": { inputPrice: 1.0, outputPrice: 4.0, contextWindow: 128000 },
  
  // OpenRouter models with pricing data
  "openai/gpt-4o-mini": { inputPrice: 0.15, outputPrice: 0.60, contextWindow: 128000 },
  "anthropic/claude-3.7-sonnet": { inputPrice: 3, outputPrice: 15, contextWindow: 200000 },
  "google/gemini-1.5-pro": { inputPrice: 1.25, outputPrice: 5, contextWindow: 2000000 },
  "anthropic/claude-3.5-sonnet": { inputPrice: 3, outputPrice: 15, contextWindow: 200000 },
  "openai/gpt-4o": { inputPrice: 2.50, outputPrice: 10, contextWindow: 128000 },
  "openai/o4-mini": { inputPrice: 1.10, outputPrice: 4.40, contextWindow: 200000 },
  "openai/o3-mini": { inputPrice: 1.10, outputPrice: 4.40, contextWindow: 200000 },
  "anthropic/claude-3-haiku": { inputPrice: 0.25, outputPrice: 1.25, contextWindow: 200000 },
};

/**
 * Get pricing information for a model
 * @param model Model identifier
 * @param provider Service provider (e.g., 'openrouter', 'openai')
 * @returns Pricing information or default pricing if model not found
 */
export function getModelPricing(model: string, provider: string = 'openai'): ModelPricing {
  const normalizedModel = model.toLowerCase();
  let modelKey = normalizedModel;
  
  // Handle different provider model naming schemes
  if (provider.toLowerCase() === 'openrouter') {
    // For OpenRouter, check prefixed format: "provider/model"
    // Look for the model name as-is in the database (openrouter format)
    if (modelPricing[modelKey]) {
      return modelPricing[modelKey];
    }
    
    // Try to find model by searching for its part in the keys
    for (const key of Object.keys(modelPricing)) {
      if (key.toLowerCase().includes(normalizedModel)) {
        return modelPricing[key];
      }
    }
    
    // If model has slashes, try to match the last part
    if (normalizedModel.includes('/')) {
      const parts = normalizedModel.split('/');
      const modelNameWithoutProvider = parts[parts.length - 1];
      
      // Try exact match with the model part
      for (const key of Object.keys(modelPricing)) {
        if (key.toLowerCase().endsWith('/' + modelNameWithoutProvider.toLowerCase())) {
          return modelPricing[key];
        }
      }
      
      // Try partial match with the model part
      for (const key of Object.keys(modelPricing)) {
        if (key.toLowerCase().includes(modelNameWithoutProvider.toLowerCase())) {
          return modelPricing[key];
        }
      }
    }
  } else {
    // For other providers, try to match with common name patterns
    // Handle OpenAI shorthand versions
    if (normalizedModel === 'gpt-4o-mini') {
      return modelPricing['openai/gpt-4o-mini'];
    } else if (normalizedModel === 'gpt-4o') {
      return modelPricing['openai/gpt-4o'];
    } else if (normalizedModel === 'o4-mini') {
      return modelPricing['openai/o4-mini'];
    } else if (normalizedModel === 'o3-mini') {
      return modelPricing['openai/o3-mini'];
    }
    
    // Try to match with Claude models
    if (normalizedModel.includes('claude-3.7')) {
      return modelPricing['anthropic/claude-3.7-sonnet'];
    } else if (normalizedModel.includes('claude-3.5')) {
      return modelPricing['anthropic/claude-3.5-sonnet'];
    } else if (normalizedModel.includes('claude-3-haiku')) {
      return modelPricing['anthropic/claude-3-haiku'];
    }
  }
  
  // Default pricing for unknown models
  return modelPricing.default;
}

/**
 * Calculate the cost for a given token usage
 * @param usedTokens Total tokens used
 * @param model Model identifier
 * @param provider Service provider (e.g., 'openrouter', 'openai')
 * @param inputRatio Ratio of input tokens to total tokens (default: 0.75)
 * @returns Estimated cost in USD
 */
export function calculateTokenCost(
  usedTokens: number, 
  model: string,
  provider: string = 'openrouter',
  inputRatio: number = 0.75
): number {
  const pricing = getModelPricing(model, provider);
  
  // Estimate input vs output tokens based on ratio
  const inputTokens = Math.floor(usedTokens * inputRatio);
  const outputTokens = usedTokens - inputTokens;
  
  // Calculate cost (convert from per 1M tokens to per token)
  const inputCost = (inputTokens / 1_000_000) * pricing.inputPrice;
  const outputCost = (outputTokens / 1_000_000) * pricing.outputPrice;
  
  return inputCost + outputCost;
}

/**
 * Get maximum context window size for a model
 * @param model Model identifier
 * @param provider Service provider (e.g., 'openrouter', 'openai')
 * @returns Maximum context window size in tokens
 */
export function getContextWindowSize(model: string, provider: string = 'openrouter'): number {
  const pricing = getModelPricing(model, provider);
  return pricing.contextWindow;
}