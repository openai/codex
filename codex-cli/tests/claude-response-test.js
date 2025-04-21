// Direct test of Claude provider response format

import Anthropic from "@anthropic-ai/sdk";
import { ClaudeProvider } from "../src/utils/providers/claude-provider";

// Create API key or exit
const apiKey = process.env.CLAUDE_API_KEY || process.env.ANTHROPIC_API_KEY;
if (!apiKey) {
  console.error("Missing CLAUDE_API_KEY or ANTHROPIC_API_KEY environment variable");
  process.exit(1);
}

async function testClaudeProvider() {
  console.log("Testing Claude provider directly...");
  
  // Create provider
  const provider = new ClaudeProvider();
  
  // Create a simple config
  const config = {
    model: "claude-3-sonnet-20240229",
    providers: {
      claude: {
        apiKey: apiKey
      }
    }
  };
  
  // Create client using provider
  const client = provider.createClient(config);
  
  // Test simple completion
  try {
    console.log("Testing simple completion...");
    
    // Create a test message
    const result = await client.responses.create({
      model: "claude-3-sonnet-20240229",
      instructions: "You are a helpful assistant.",
      input: [
        {
          role: "user",
          content: [
            {
              type: "input_text", 
              text: "Say hello and tell me today's date"
            }
          ]
        }
      ],
      stream: false
    });
    
    console.log("\nClaude response:");
    console.log(JSON.stringify(result, null, 2));
    
    if (result.output && result.output.length > 0) {
      console.log("\nFormatted response:");
      
      // Extract text content
      for (const item of result.output) {
        if (item.type === "message" && item.content) {
          for (const content of item.content) {
            if (content.type === "output_text") {
              console.log(`assistant: ${content.text}`);
            }
          }
        }
      }
    }
  } catch (error) {
    console.error("Error testing Claude provider:", error);
    process.exit(1);
  }
  
  console.log("\n✅ Claude provider test successful");
}

testClaudeProvider();