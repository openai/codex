import Ollama from 'ollama';

export const OLLAMA_BASE_URL = process.env["OLLAMA_BASE_URL"] || "http://localhost:11434";
export const OLLAMA_DEFAULT_MODEL = process.env["OLLAMA_DEFAULT_MODEL"] || "llama2";

export interface OllamaConfig {
  baseUrl?: string;
  model: string;
}

export class OllamaClient {
  private model: string;

  constructor(config: OllamaConfig) {
    this.model = config.model;
  }

  async generateResponse(prompt: string, options?: any) {
    try {
      const response = await Ollama.chat({
        model: this.model,
        messages: [{ role: 'user', content: prompt }],
        stream: true,
        ...options
      });

      return response;
    } catch (error) {
      console.error('Error generating response from Ollama:', error);
      throw error;
    }
  }

  async listModels() {
    try {
      const models = await Ollama.list();
      return models;
    } catch (error) {
      console.error('Error listing Ollama models:', error);
      throw error;
    }
  }
} 