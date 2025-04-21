/**
 * Minimal Claude Provider Fix
 * 
 * This file contains the minimal changes needed to fix the Claude provider issues.
 * These functions can be copied into the main claude-provider.ts file.
 */

/**
 * Normalize a shell command to the expected format
 * This is a key function that handles various edge cases in command formats
 * 
 * @param command The command to normalize (can be string, array, or undefined)
 * @returns A properly formatted command array
 */
export function normalizeShellCommand(command: any): string[] {
  // Handle empty or undefined command
  if (!command) {
    console.log(`Claude provider: Empty command detected, using default ls command`);
    return ["ls", "-ltr"];  // Changed to ls -ltr for testing
  }
  
  // If command is a string
  if (typeof command === 'string') {
    // Handle empty string
    if (command.trim() === '') {
      console.log(`Claude provider: Empty string command, using default ls command`);
      return ["ls", "-ltr"];
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
      return ["ls", "-ltr"];
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
  return ["ls", "-ltr"];
}

/**
 * Process shell tool input to ensure proper format
 * This handles completely empty input objects which was causing issues
 * 
 * @param toolInput The input from a shell tool call
 * @returns Properly formatted tool arguments
 */
export function processShellToolInput(toolInput: any): { command: string[], workdir?: string } {
  // Handle completely empty or missing input
  if (!toolInput || typeof toolInput !== 'object' || Object.keys(toolInput).length === 0) {
    console.log(`Claude provider: Empty or invalid tool input, using default command`);
    return {
      command: ["ls", "-ltr"],
      workdir: process.cwd()
    };
  }
  
  // Extract command from input
  const command = normalizeShellCommand(toolInput.command);
  
  // Ensure workdir is present
  const workdir = toolInput.workdir || process.cwd();
  
  return {
    command,
    workdir
  };
}