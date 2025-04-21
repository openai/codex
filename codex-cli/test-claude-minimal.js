/**
 * Test Script for Minimal Claude Provider
 * 
 * This script tests the minimal Claude provider implementation
 * for proper handling of shell command formats and empty inputs.
 */

import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import fs from 'fs';
import Anthropic from '@anthropic-ai/sdk';

// Simulate tool calls
const mockShellTool = {
  type: "function",
  name: "shell",
  description: "Runs a shell command, and returns its output.",
  strict: false,
  parameters: {
    type: "object",
    properties: {
      command: { 
        type: "array", 
        items: { type: "string" } 
      },
      workdir: {
        type: "string",
        description: "The working directory for the command."
      }
    },
    required: ["command"]
  }
};

// Prompt with clear instructions for shell commands
const systemPrompt = `You are a helpful assistant that can run shell commands.

CRITICAL INSTRUCTIONS FOR SHELL COMMANDS:

I want you to use the shell tool directly in your responses. When you need to run a command, DO NOT describe what you want to do - just USE the shell tool.

To use the shell tool, DO NOT write any text like "I'll use the shell tool to...". Instead, DIRECTLY call the tool with appropriate parameters.

ALWAYS format shell commands as follows:

1. For ANY command with pipes, redirects, wildcards, or shell features:
   { "command": ["bash", "-c", "your command here with pipes or redirects"] }

2. For simple commands:
   { "command": ["command", "arg1", "arg2"] }

EXAMPLES OF CORRECT USAGE:
- Calculator: { "command": ["bash", "-c", "echo '1+1' | bc"] }
- File search: { "command": ["bash", "-c", "find . -name '*.js' | grep 'test'"] }
- Directory listing: { "command": ["ls", "-la"] }
- Echo: { "command": ["echo", "hello world"] }

IMPORTANT: 
- The command MUST ALWAYS be an ARRAY, never a string
- For complex commands, ALWAYS use ["bash", "-c", "command"] format
- Command is required - never send an empty command
- NEVER explain what you're going to do - just use the tool directly`;

/**
 * Process shell tool input to ensure proper format
 * 
 * @param {any} toolInput The input from a shell tool call
 * @returns {{ command: string[], workdir?: string }} Properly formatted tool arguments
 */
function processShellToolInput(toolInput) {
  // Handle completely empty or missing input
  if (!toolInput || typeof toolInput !== 'object') {
    console.log(`Test function: Empty or invalid tool input, using default command`);
    return {
      command: ["ls", "-la"],
      workdir: process.cwd()
    };
  }
  
  // Extract command from input
  let command = toolInput.command;
  
  // Handle empty command
  if (!command) {
    console.log(`Test function: Empty command detected, using default ls command`);
    command = ["ls", "-la"];
  }
  // Handle string command
  else if (typeof command === 'string') {
    // Handle empty string
    if (command.trim() === '') {
      console.log(`Test function: Empty string command, using default ls command`);
      command = ["ls", "-la"];
    }
    // Check if the command string is actually a JSON string of an array
    else if (command.startsWith('[') && command.endsWith(']')) {
      try {
        const parsedCommand = JSON.parse(command);
        if (Array.isArray(parsedCommand)) {
          console.log(`Test function: Detected JSON string containing an array, parsing it: ${command}`);
          command = parsedCommand;
          
          // Check if parsed array needs bash -c wrapping
          if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
            const cmdStr = parsedCommand.join(' ');
            console.log(`Test function: Wrapping parsed array in bash -c: ${cmdStr}`);
            command = ["bash", "-c", cmdStr];
          }
        } else {
          // Not an array after parsing, treat as regular string
          console.log(`Test function: Parsed JSON is not an array, treating as regular string: ${command}`);
          command = ["bash", "-c", command];
        }
      } catch (parseError) {
        // Not valid JSON, treat as regular string
        console.log(`Test function: Failed to parse command as JSON, treating as regular string: ${command}`);
        command = ["bash", "-c", command];
      }
    } else {
      // Regular string command
      console.log(`Test function: Converting string command to bash -c: ${command}`);
      command = ["bash", "-c", command];
    }
  }
  // Handle array command
  else if (Array.isArray(command)) {
    // Handle empty array
    if (command.length === 0) {
      console.log(`Test function: Empty command array, using default ls command`);
      command = ["ls", "-la"];
    }
    // Check if array needs bash -c wrapping
    else if (!(command[0] === "bash" && command[1] === "-c")) {
      const cmdStr = command.join(' ');
      
      // Check if command contains shell special characters
      const needsBashC = cmdStr.includes('|') || 
                        cmdStr.includes('>') || 
                        cmdStr.includes('<') || 
                        cmdStr.includes('*') || 
                        cmdStr.includes('?') || 
                        cmdStr.includes('$') ||
                        cmdStr.includes('&&') ||
                        cmdStr.includes('||');
      
      if (needsBashC) {
        console.log(`Test function: Converting command array to bash -c: ${cmdStr}`);
        command = ["bash", "-c", cmdStr];
      }
    }
  }
  
  // Ensure workdir is present
  const workdir = toolInput.workdir || process.cwd();
  
  return {
    command,
    workdir
  };
}

/**
 * Run a test with Claude using the shell tool
 */
async function runClaudeTest() {
  console.log("=== Testing Claude Shell Command Handling ===");
  
  // Get API key from environment
  const apiKey = process.env.CLAUDE_API_KEY || process.env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    console.error("No Claude API key found. Please source ~/.bashrc or set CLAUDE_API_KEY/ANTHROPIC_API_KEY environment variable.");
    process.exit(1);
  }
  
  const client = new Anthropic({
    apiKey: apiKey,
  });
  
  // Define a shell tool for Claude
  const tools = [
    {
      name: "shell",
      description: "Runs a shell command, and returns its output.",
      input_schema: {
        type: "object",
        properties: {
          command: { 
            type: "array", 
            items: { type: "string" },
            description: "The command to execute as an array of strings."
          },
          workdir: {
            type: "string",
            description: "The working directory for the command."
          }
        },
        required: ["command"]
      }
    }
  ];
  
  // Test messages to check different command formatting challenges
  const testMessages = [
    "List all files in the current directory",
    "Calculate 1+1 using bc",
    "Count the number of JavaScript files in the current directory"
  ];
  
  for (const message of testMessages) {
    console.log(`\n----- Testing: "${message}" -----`);
    
    try {
      // Make request to Claude
      const response = await client.messages.create({
        model: "claude-3-5-sonnet-20240620",
        system: systemPrompt,
        max_tokens: 1024, 
        messages: [{ role: "user", content: message }],
        tools: tools
      });
      
      // Find tool use blocks
      const toolUseBlocks = response.content.filter(block => block.type === 'tool_use');
      
      if (toolUseBlocks.length > 0) {
        console.log("✅ Claude responded with a tool call");
        
        for (const block of toolUseBlocks) {
          console.log("\nOriginal Tool Call:");
          console.log(`- Name: ${block.name}`);
          console.log(`- Input:`, JSON.stringify(block.input, null, 2));
          
          // Process the tool call with our function
          const processedInput = processShellToolInput(block.input);
          console.log("\nProcessed Tool Call:");
          console.log(`- Command:`, JSON.stringify(processedInput.command, null, 2));
          console.log(`- Workdir:`, processedInput.workdir);
          
          // Verify the processed command is valid
          const isValid = Array.isArray(processedInput.command) && 
                          processedInput.command.length > 0 &&
                          processedInput.workdir;
          
          if (isValid) {
            console.log(`\n✅ Successfully processed command: ${JSON.stringify(processedInput.command)}`);
          } else {
            console.log(`\n❌ Failed to process command correctly`);
          }
          
          // Check for empty input - one of the main issues we're testing
          if (!block.input || Object.keys(block.input).length === 0) {
            console.log(`\n🔎 EMPTY INPUT TEST CASE DETECTED`);
            console.log(`Original input was empty: ${JSON.stringify(block.input)}`);
            console.log(`Processed to: ${JSON.stringify(processedInput)}`);
          }
        }
      } else {
        console.log("❌ Claude did not use a tool call");
        
        // Check if there's text content
        const textContent = response.content
          .filter(block => block.type === 'text')
          .map(block => block.text)
          .join('\n');
          
        console.log("Text response:", textContent);
      }
    } catch (error) {
      console.error(`Error: ${error.message}`);
    }
  }
  
  // Special test case for empty input
  console.log("\n----- Special Test: Empty Input {} -----");
  
  const emptyInput = { id: "tool_123", name: "shell", input: {} };
  console.log(`Empty input test case: ${JSON.stringify(emptyInput)}`);
  
  const processedEmptyInput = processShellToolInput(emptyInput.input);
  console.log(`Processed empty input: ${JSON.stringify(processedEmptyInput)}`);
  
  const isValid = Array.isArray(processedEmptyInput.command) && 
                  processedEmptyInput.command.length > 0 &&
                  processedEmptyInput.workdir;
                  
  if (isValid) {
    console.log(`✅ Successfully processed empty input to valid command: ${JSON.stringify(processedEmptyInput.command)}`);
  } else {
    console.log(`❌ Failed to process empty input correctly`);
  }
}

// Run the tests
runClaudeTest().catch(err => {
  console.error("Test failed:", err);
});