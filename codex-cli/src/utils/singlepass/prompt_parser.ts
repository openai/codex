export interface ParsedPrompt {
  cleanedPrompt: string;
  hiddenPatterns: string[];
}

/**
 * Parse the user's prompt to extract hidden file specifications
 * Format: #hidden: pattern1, pattern2, ...
 * Support multiple hidden pattern in a prompt
 */
export function parsePromptForHiddenFiles(prompt: string): ParsedPrompt {
  const hiddenRegex = /#hidden:\s*(.*?)(?=$|\n)/g;
  let hiddenPatterns: string[] = [];

  // Replace all #hidden: directives and collect the patterns
  const cleanedPrompt = prompt.replace(hiddenRegex, (_, patternList) => {
    const patterns = patternList
      .split(",")
      .map((p: string) => p.trim())
      .filter((p: string) => p.length > 0);

    hiddenPatterns = [...hiddenPatterns, ...patterns];
    return "";
  });

  return {
    cleanedPrompt: cleanedPrompt.replace(/\n{2,}/g, "\n").trim(),
    hiddenPatterns,
  };
}
