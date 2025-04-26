/**
 * Pattern to match various "AI!" style trigger comments with possible instructions
 * Supports multiple single-line programming language comment styles:
 *   - Double slash comment (C, C++, JavaScript, TypeScript, Java, etc.)
 *   - Hash comment (Python, Ruby, Perl, Shell scripts, YAML, etc.)
 *   - Double dash comment (SQL, Haskell, Lua)
 *   - Semicolon comment (Lisp, Clojure, Assembly)
 *   - Single quote comment (VB, VBA)
 *   - Percent comment (LaTeX, Matlab, Erlang)
 *   - REM comment (Batch files)
 *
 * Examples:
 * - "// what does this function do, AI?"
 * - "# Fix this code, AI!"
 * - "-- Optimize this query, AI!"
 */

export const TRIGGER_PATTERN =
  /(?:\/\/|#|--|;|'|%|REM)\s*(.*?)(?:,\s*)?AI[!?]/i;

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

  // Extract the instruction from the capture groups for different comment styles
  // There are multiple capture groups for different comment syntaxes
  // Find the first non-undefined capture group
  let instruction =
    Array.from(
      { length: triggerMatch.length - 1 },
      (_, i) => triggerMatch[i + 1],
    ).find((group) => group !== undefined) || "fix or improve this code";
    
  // Remove any comment prefixes that might have been captured
  instruction = instruction.replace(/^(?:\/\/|#|--|;|'|%|REM)\s*/, "");
  
  return { context, instruction };
}

