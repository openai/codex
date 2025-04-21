// Simple script to verify Claude API is working

// Import Anthropic directly
import Anthropic from "@anthropic-ai/sdk";

// Create Anthropic client from environment variable
const apiKey = process.env.CLAUDE_API_KEY || process.env.ANTHROPIC_API_KEY;
if (!apiKey) {
  console.error("Missing CLAUDE_API_KEY or ANTHROPIC_API_KEY environment variable");
  process.exit(1);
}

console.log("Testing direct Claude API call...");

const anthropic = new Anthropic({ apiKey });

// Make a simple request to verify connectivity
async function testClaude() {
  try {
    const response = await anthropic.messages.create({
      model: "claude-3-sonnet-20240229",
      max_tokens: 100,
      messages: [
        { role: "user", content: "Say hello and confirm you are Claude" }
      ],
      system: "You are a helpful AI assistant."
    });

    console.log("Response from Claude API:", response.content[0].text);
    return true;
  } catch (error) {
    console.error("Error calling Claude API:", error);
    return false;
  }
}

// Run the test
testClaude()
  .then(success => {
    if (success) {
      console.log("✅ Claude API test successful");
    } else {
      console.log("❌ Claude API test failed");
      process.exit(1);
    }
  });