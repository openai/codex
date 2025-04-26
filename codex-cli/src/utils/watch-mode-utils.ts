import { loadConfig } from "./config";

/**
 * Custom trigger patterns for watch mode
 * 
 * Users can define their own trigger patterns in config.json:
 * ```json
 * {
 *   "watchMode": {
 *     "triggerPattern": "/(?:\\/\\/|#)\\s*AI:(TODO|FIXME)\\s+(.*)/i"
 *   }
 * }
 * ```
 * 
 * The pattern MUST include at least one capture group that will contain the instruction.
 * 
 * Examples:
 * 
 * Default pattern (single-line comments ending with AI! or AI?):
 * - "// what does this function do, AI?"
 * - "# Fix this code, AI!"
 * - "-- Optimize this query, AI!"
 * 
 * Custom pattern for task management:
 * - "// AI:TODO fix this bug"
 * - "// AI:FIXME handle error case"
 * - "# AI:BUG this crashes with null input"
 * 
 * Custom pattern with different keyword:
 * - "// codex! fix this"
 * - "# codex? what does this function do"
 */

// Default trigger pattern
const DEFAULT_TRIGGER_PATTERN = '/(?:\\/\\/|#|--|;|\'|%|REM)\\s*(.*?)(?:,\\s*)?AI[!?]/i';

/**
 * Get the configured trigger pattern from config.json or use the default
 * @returns A RegExp object for the trigger pattern
 */
export function getTriggerPattern(): RegExp {
  const config = loadConfig();
  
  // Get the pattern string from config or use default
  const patternString = config.watchMode?.triggerPattern || DEFAULT_TRIGGER_PATTERN;
  
  try {
    // Parse the regex from the string - first remove enclosing slashes and extract flags
    const match = patternString.match(/^\/(.*)\/([gimuy]*)$/);
    
    if (match) {
      const [, pattern, flags] = match;
      return new RegExp(pattern, flags);
    } else {
      // If not in /pattern/flags format, try to use directly as a RegExp
      return new RegExp(patternString, 'i');
    }
  } catch (error) {
    console.warn(`Invalid trigger pattern in config: ${patternString}. Using default.`);
    // Parse default pattern
    const match = DEFAULT_TRIGGER_PATTERN.match(/^\/(.*)\/([gimuy]*)$/);
    const [, pattern, flags] = match!;
    return new RegExp(pattern, flags);
  }
}

/**
 * Function to find all trigger matches in a file content
 * Uses the configured trigger pattern from config.json
 */
export function findAllTriggers(content: string): Array<RegExpMatchArray> {
  const matches: Array<RegExpMatchArray> = [];
  const pattern = getTriggerPattern();
  // We need to ensure the global flag is set for .exec() to work properly
  const regex = new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : pattern.flags + 'g');

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
  contextSize = 15,
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

  // Get instruction from capture groups
  // For custom patterns, check all capture groups and use the last non-empty one
  // This allows patterns like /AI:(TODO|FIXME)\s+(.*)/ where we want the second group
  // For the default pattern, it will be the first capture group
  const captureGroups = Array.from(
    { length: triggerMatch.length - 1 },
    (_, i) => triggerMatch[i + 1]
  ).filter(group => group !== undefined);
  
  // Use the last non-empty capture group as the instruction
  // For simple patterns with one capture, this will be that capture
  // For patterns with multiple captures (like task types), this will be the actual instruction
  let instruction = captureGroups.length > 0 
    ? captureGroups[captureGroups.length - 1] 
    : "fix or improve this code";
  
  // Remove any comment prefixes that might have been captured
  instruction = instruction.replace(/^(?:\/\/|#|--|;|'|%|REM)\s*/, "");
  
  return { context, instruction };
}

