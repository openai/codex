/**
 * Simple test for Claude provider core functions
 * 
 * This script manually tests the core functions for handling tool calls in the Claude provider
 */

/**
 * Normalize a shell command to the expected format
 * @param command The command to normalize (can be string, array, or undefined)
 * @returns A properly formatted command array
 */
function normalizeShellCommand(command) {
  // Handle empty or undefined command
  if (!command) {
    console.log(`Claude provider: Empty command detected, using default ls command`);
    return ["ls", "-la"];
  }
  
  // If command is a string
  if (typeof command === 'string') {
    // Handle empty string
    if (command.trim() === '') {
      console.log(`Claude provider: Empty string command, using default ls command`);
      return ["ls", "-la"];
    }
    
    // Check if the command string is actually a JSON string of an array
    if (command.startsWith('[') && command.endsWith(']')) {
      try {
        const parsedCommand = JSON.parse(command);
        if (Array.isArray(parsedCommand)) {
          console.log(`Claude provider: Detected JSON string containing an array, parsing it: ${command}`);
          
          // Now check if the parsed array needs bash -c wrapping
          if (!(parsedCommand[0] === "bash" && parsedCommand[1] === "-c")) {
            const cmdStr = parsedCommand.join(' ');
            console.log(`Claude provider: Wrapping parsed array in bash -c: ${cmdStr}`);
            return ["bash", "-c", cmdStr];
          }
          
          return parsedCommand;
        }
      } catch (parseError) {
        // Not valid JSON, treat as regular string
        console.log(`Claude provider: Failed to parse command as JSON, using bash -c: ${command}`);
      }
    }
    
    // For all other strings, wrap in bash -c
    console.log(`Claude provider: Converting command string to bash -c: ${command}`);
    return ["bash", "-c", command];
  }
  
  // If command is an array
  if (Array.isArray(command)) {
    // Handle empty array
    if (command.length === 0) {
      console.log(`Claude provider: Empty command array, using default ls command`);
      return ["ls", "-la"];
    }
    
    // If not already in bash -c format and contains shell special characters
    // or seems to need shell features, wrap it
    if (!(command[0] === "bash" && command[1] === "-c")) {
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
        console.log(`Claude provider: Converting command array to bash -c: ${cmdStr}`);
        return ["bash", "-c", cmdStr];
      }
    }
    
    // Return the array as is
    return command;
  }
  
  // For any other type, return default command
  console.log(`Claude provider: Unknown command type (${typeof command}), using default ls command`);
  return ["ls", "-la"];
}

/**
 * Process shell tool input to ensure proper format
 * @param toolInput The shell tool input
 * @returns Properly formatted tool arguments
 */
function processShellToolInput(toolInput) {
  // Handle completely empty or missing input
  if (!toolInput || typeof toolInput !== 'object') {
    console.log(`Claude provider: Empty or invalid tool input, using default command`);
    return {
      command: ["ls", "-la"],
      workdir: process.cwd()
    };
  }
  
  // Extract command from input
  let command = normalizeShellCommand(toolInput.command);
  
  // Ensure workdir is present
  const workdir = toolInput.workdir || process.cwd();
  
  return {
    command,
    workdir
  };
}

/**
 * Parse a tool call from Claude format to common format
 * @param toolCall Claude tool call
 * @returns Normalized tool call
 */
function parseToolCall(toolCall) {
  // Log the raw tool call
  console.log(`Claude provider: Parsing tool call: ${JSON.stringify(toolCall)}`);
  
  // Extract basic information from Claude's format
  const toolId = toolCall.id || `tool_${Date.now()}`;
  const toolName = toolCall.name || "unknown";
  
  // Get the raw input from Claude's format
  let toolArgs = toolCall.input || {};
  
  // Special handling for shell commands to ensure they work correctly
  if (toolName === "shell") {
    console.log(`Claude provider: Processing shell command`);
    
    // Process shell command to ensure it's in the correct format
    toolArgs = processShellToolInput(toolArgs);
  }
  
  console.log(`Claude provider: Parsed tool call: ${toolName}, args: ${JSON.stringify(toolArgs)}`);
  
  return {
    id: toolId,
    name: toolName,
    arguments: toolArgs,
  };
}

/**
 * Main test function
 */
function runTests() {
  console.log("=== Testing Claude Provider Tool Handling ===");
  
  // Testing problematic cases
  const testCases = [
    { 
      name: "Empty object input", 
      input: { 
        id: "tool_123", 
        name: "shell", 
        input: {} 
      }
    },
    { 
      name: "String command", 
      input: { 
        id: "tool_123", 
        name: "shell", 
        input: { 
          command: "ls -la" 
        } 
      }
    },
    { 
      name: "Array command with pipe", 
      input: { 
        id: "tool_123", 
        name: "shell", 
        input: { 
          command: ["find", ".", "-name", "*.js", "|", "grep", "test"] 
        } 
      }
    },
    { 
      name: "JSON string command", 
      input: { 
        id: "tool_123", 
        name: "shell", 
        input: { 
          command: '["ls", "-la"]' 
        } 
      }
    }
  ];
  
  console.log("\n=== Testing parseToolCall ===");
  
  for (const testCase of testCases) {
    console.log(`\nTest case: ${testCase.name}`);
    console.log(`Input: ${JSON.stringify(testCase.input)}`);
    
    try {
      // Call the parse function
      const result = parseToolCall(testCase.input);
      console.log(`Output: ${JSON.stringify(result)}`);
      
      // Check if the result has a valid command array
      if (!result.arguments || !Array.isArray(result.arguments.command)) {
        console.log(`Result: ❌ FAIL - Missing or invalid command array`);
        continue;
      }
      
      // Check if the result has a workdir
      const hasWorkdir = typeof result.arguments.workdir === 'string';
      if (!hasWorkdir) {
        console.log(`Result: ❌ FAIL - Missing workdir property`);
        continue;
      }
      
      console.log(`Result: ✅ PASS`);
    } catch (error) {
      console.log(`Error: ${error.message}`);
      console.log(`Result: ❌ FAIL - threw exception`);
    }
  }
  
  console.log("\n=== All Tests Completed ===");
}

// Run the tests
runTests();