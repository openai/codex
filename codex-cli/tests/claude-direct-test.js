// Direct test of Claude API using Anthropic SDK

import Anthropic from "@anthropic-ai/sdk";

// Create API key or exit
const apiKey = process.env.CLAUDE_API_KEY || process.env.ANTHROPIC_API_KEY;
if (!apiKey) {
  console.error("Missing CLAUDE_API_KEY or ANTHROPIC_API_KEY environment variable");
  process.exit(1);
}

async function testClaudeDirectly() {
  console.log("Testing Claude directly with Anthropic SDK...");
  
  // Create client
  const client = new Anthropic({
    apiKey: apiKey
  });
  
  // Test message creation
  try {
    console.log("Sending request to Claude API...");
    
    const message = await client.messages.create({
      model: "claude-3-sonnet-20240229",
      max_tokens: 1000,
      system: "You are a helpful assistant.",
      messages: [
        {
          role: "user",
          content: "Say hello and tell me today's date"
        }
      ]
    });
    
    console.log("\nClaude API response:");
    console.log(JSON.stringify(message, null, 2));
    
    // Extract message content
    if (message.content && message.content.length > 0) {
      console.log("\nResponse content:");
      for (const content of message.content) {
        if (content.type === "text") {
          console.log(`assistant: ${content.text}`);
        }
      }
    }
  } catch (error) {
    console.error("Error testing Claude API:", error);
    process.exit(1);
  }
  
  // Test tools (shell function)
  try {
    console.log("\nTesting Claude with tools...");
    
    const toolMessage = await client.messages.create({
      model: "claude-3-sonnet-20240229",
      max_tokens: 1000,
      system: "You are a helpful coding assistant.",
      messages: [
        {
          role: "user",
          content: "Run ls command to show files in the current directory"
        }
      ],
      tools: [
        {
          name: "shell",
          description: "Runs a shell command and returns its output",
          input_schema: {
            type: "object",
            properties: {
              command: { 
                type: "array", 
                items: { type: "string" },
                description: "The command to run"
              },
              workdir: { 
                type: "string", 
                description: "Working directory for the command"
              }
            },
            required: ["command"]
          }
        }
      ]
    });
    
    console.log("\nClaude API tool response:");
    console.log(JSON.stringify(toolMessage, null, 2));
    
    // Check for tool use
    const toolUses = toolMessage.content.filter(content => content.type === "tool_use");
    if (toolUses.length > 0) {
      console.log("\nTool uses detected:");
      for (const toolUse of toolUses) {
        console.log(`Tool: ${toolUse.name}`);
        console.log(`Input: ${JSON.stringify(toolUse.input, null, 2)}`);
      }
    } else {
      console.log("\nNo tool uses detected");
    }
  } catch (error) {
    console.error("Error testing Claude API with tools:", error);
  }
  
  console.log("\n✅ Claude direct test successful");
}

testClaudeDirectly();