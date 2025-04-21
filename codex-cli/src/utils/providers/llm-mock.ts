/**
 * LLMMock - A mock implementation of an LLM client for testing
 * This provides a mock client that can be used for testing provider implementations
 */

export class LLMMock {
  apiKey: string;
  baseURL?: string;
  timeout?: number;
  defaultHeaders?: Record<string, string>;
  
  // Add responses property to mirror OpenAI's API structure
  responses: {
    create: (params: any) => Promise<any>;
  };
  
  // Add messages property for Claude-style API
  messages: {
    create: (params: any) => Promise<any>;
    stream: (params: any) => any;
  };

  constructor(options: { 
    apiKey: string, 
    baseURL?: string, 
    timeout?: number,
    defaultHeaders?: Record<string, string>
  }) {
    this.apiKey = options.apiKey;
    this.baseURL = options.baseURL;
    this.timeout = options.timeout;
    this.defaultHeaders = options.defaultHeaders;
    
    // Initialize the messages API
    this.messages = {
      create: async (params: any) => {
        // This would be a real API call in production code
        return { id: "msg_123", content: [] };
      },
      stream: async (params: any) => {
        // Mock streaming interface
        return {
          [Symbol.asyncIterator]: async function* () {
            yield { type: "content_block_start", content_block: { type: "text" } };
            yield { type: "content_block_delta", delta: { text: "Response from Mock LLM" } };
            yield { type: "content_block_stop" };
            yield { type: "message_stop" };
          }
        };
      }
    };
    
    // Create a responses property that maps to messages to be compatible with OpenAI
    this.responses = {
      create: (params: any) => {
        console.log("LLMMock: mapping responses.create to messages.create");
        
        // For streaming requests, we need to return an async iterable
        if (params.stream) {
          console.log("LLMMock: creating streaming response");
          
          // Return an object that implements the AsyncIterable interface
          return {
            [Symbol.asyncIterator]: async function* () {
              // Mock response events that match what OpenAI's client expects
              
              // First yield response item with text
              // Print to console directly so we can see it works
              console.log("assistant: Hello! This is a response from the LLM mock implementation.");
              
              yield { 
                type: "response.output_item.done", 
                item: {
                  type: "message",
                  role: "assistant",
                  content: [
                    {
                      type: "output_text",
                      text: "Hello! This is a response from the LLM mock implementation."
                    }
                  ]
                }
              };
              
              // Then yield completion event
              yield { 
                type: "response.completed", 
                response: {
                  id: "resp_" + Date.now(),
                  status: "completed",
                  output: [
                    {
                      type: "message",
                      role: "assistant",
                      content: [
                        {
                          type: "output_text",
                          text: "Hello! This is a response from the LLM mock implementation."
                        }
                      ]
                    }
                  ]
                }
              };
            },
            controller: {
              abort: () => console.log("LLMMock: aborting stream")
            }
          };
        }
        
        // For non-streaming requests
        return this.messages.create({
          model: params.model,
          messages: params.input || [],
          system: params.instructions,
          stream: false,
          tools: params.tools,
        });
      }
    };
  }

  // Mock API for demonstration purposes
  async listModels() {
    return {
      data: [
        { id: "mock-model-large" },
        { id: "mock-model-medium" },
        { id: "mock-model-small" },
      ]
    };
  }
}