// Simple test of Claude provider with visible output

import Anthropic from "@anthropic-ai/sdk";
import { ClaudeProvider } from "../src/utils/providers/claude-provider";

// Create direct test of our provider implementation
async function testClaudeProvider() {
  console.log("TESTING CLAUDE PROVIDER:");
  console.log("========================");

  try {
    // Create a provider instance
    const provider = new ClaudeProvider();
    console.log("Created ClaudeProvider instance");

    // Create a simple config
    const config = {
      model: "claude-3-sonnet-20240229",
      providers: {
        claude: {
          apiKey: process.env.CLAUDE_API_KEY || process.env.ANTHROPIC_API_KEY
        }
      }
    };

    // Get the client
    console.log("Creating Claude client via provider.createClient()...");
    const client = provider.createClient(config);
    console.log("Client created successfully");

    // Send a simple completion request
    console.log("\nSending completion request to Claude via our provider...");
    const response = await client.responses.create({
      model: "claude-3-sonnet-20240229",
      input: [{ 
        role: "user", 
        content: [{ type: "input_text", text: "What is 2+2?" }]
      }],
      instructions: "You are a helpful assistant",
      stream: false
    });

    // Show the complete response
    console.log("\nCOMPLETE RESPONSE FROM CLAUDE PROVIDER:");
    console.log(JSON.stringify(response, null, 2));

    console.log("\nTEST SUCCESSFUL ✅");
  } catch (error) {
    console.error("\nTEST FAILED ❌");
    console.error("Error:", error);
  }
}

testClaudeProvider();