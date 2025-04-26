/**
 * Pattern to match various "AI!" style trigger comments with possible instructions
 * This will match patterns like:
 * - "// what does this function do, AI?"
 * - "// change this variable name to something more precise, AI!"
 * - "# fix this code, AI!"
 */
export const TRIGGER_PATTERN =
  /\/\/\s*(.*),?\s*AI[!?]|#\s*(.*),?\s*AI[!?]|\/\*\s*(.*),?\s*AI[!?]\s*\*\//;

/**
 * Function to find all AI trigger matches in a file content
 */
export function findAllTriggers(content: string): Array<RegExpMatchArray> {
  const matches: Array<RegExpMatchArray> = [];
  const regex = new RegExp(TRIGGER_PATTERN, "g");

  let match;
  while ((match = regex.exec(content)) != null) {
    matches.push(match);
  }

  return matches;
}

/**
 * Function to extract context around an AI trigger
 */
export function extractContextAroundTrigger(
  content: string,
  triggerMatch: RegExpMatchArray,
  contextSize = 15
): { context: string; instruction: string } {
  // Get the lines of the file
  const lines = content.split("\n");

  // Find the line number of the trigger
  const triggerPos =
    content.substring(0, triggerMatch.index).split("\n").length - 1;

  // Calculate start and end lines for context
  const startLine = Math.max(0, triggerPos - contextSize);
  const endLine = Math.min(lines.length - 1, triggerPos + contextSize);

  // Extract the context lines
  const contextLines = lines.slice(startLine, endLine + 1);

  // Join the context lines back together
  const context = contextLines.join("\n");

  // Extract the instruction from the capture groups
  // The regex has 3 capture groups for different comment styles:
  // Group 1: // instruction AI!
  // Group 2: # instruction AI!
  // Group 3: /* instruction AI! */
  const instruction =
    triggerMatch[1] ||
    triggerMatch[2] ||
    triggerMatch[3] ||
    "fix or improve this code";

  return { context, instruction };
}