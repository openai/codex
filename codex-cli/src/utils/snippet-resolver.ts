import { getSnippet } from "./snippet-storage.js";

export interface SnippetResolutionResult {
  resolvedPrompt: string;
  replacedSnippets: Array<{
    label: string;
    found: boolean;
    originalText: string;
  }>;
  warnings: Array<string>;
}

/**
 * Regular expression to match snippet: <label> patterns
 * Matches: snippet: label, snippet:label, snippet: label-name_123
 * Captures the label part for replacement
 */
const SNIPPET_PATTERN = /snippet:\s*([a-zA-Z0-9_-]+)/g;

/**
 * Resolve snippet references in a prompt string
 * @param prompt The original prompt containing snippet references
 * @returns Object containing the resolved prompt and metadata
 */
export function resolveSnippetsInPrompt(
  prompt: string,
): SnippetResolutionResult {
  const result: SnippetResolutionResult = {
    resolvedPrompt: prompt,
    replacedSnippets: [],
    warnings: [],
  };

  // Reset regex lastIndex to ensure consistent behavior
  SNIPPET_PATTERN.lastIndex = 0;

  // Find all snippet references
  const matches: Array<{ match: string; label: string; index: number }> = [];
  let match: RegExpExecArray | null;

  while ((match = SNIPPET_PATTERN.exec(prompt)) != null) {
    if (match[1]) {
      matches.push({
        match: match[0],
        label: match[1],
        index: match.index,
      });
    }
  }

  // If no snippets found, return original prompt
  if (matches.length === 0) {
    return result;
  }

  // Process matches in reverse order to maintain correct indices during replacement
  const sortedMatches = matches.sort((a, b) => b.index - a.index);
  let resolvedPrompt = prompt;

  for (const { match, label, index } of sortedMatches) {
    const snippetResult = getSnippet(label);

    const replacementInfo = {
      label,
      found: snippetResult.success,
      originalText: match,
    };

    result.replacedSnippets.unshift(replacementInfo); // Keep original order

    if (snippetResult.success && snippetResult.snippet) {
      // Replace the snippet reference with the actual code
      const before = resolvedPrompt.substring(0, index);
      const after = resolvedPrompt.substring(index + match.length);
      resolvedPrompt = before + snippetResult.snippet.code + after;
    } else {
      // Snippet not found - add warning but keep the reference
      result.warnings.push(
        `Snippet "${label}" not found. Reference kept as-is.`,
      );
    }
  }

  result.resolvedPrompt = resolvedPrompt;
  return result;
}

/**
 * Check if a prompt contains snippet references
 * @param prompt The prompt to check
 * @returns True if snippet references are found
 */
export function hasSnippetReferences(prompt: string): boolean {
  SNIPPET_PATTERN.lastIndex = 0;
  return SNIPPET_PATTERN.test(prompt);
}

/**
 * Get all snippet labels referenced in a prompt
 * @param prompt The prompt to scan
 * @returns Array of unique snippet labels found
 */
export function getReferencedSnippetLabels(prompt: string): Array<string> {
  SNIPPET_PATTERN.lastIndex = 0;
  const labels: Set<string> = new Set();
  let match: RegExpExecArray | null;

  while ((match = SNIPPET_PATTERN.exec(prompt)) != null) {
    if (match[1]) {
      labels.add(match[1]);
    }
  }

  return Array.from(labels);
}

/**
 * Preview what would be resolved without actually doing the replacement
 * Useful for showing users what will happen before processing
 * @param prompt The prompt to preview
 * @returns Preview information
 */
export function previewSnippetResolution(prompt: string): {
  hasSnippets: boolean;
  referencedLabels: Array<string>;
  existingSnippets: Array<string>;
  missingSnippets: Array<string>;
} {
  const hasSnippets = hasSnippetReferences(prompt);
  const referencedLabels = getReferencedSnippetLabels(prompt);

  const existingSnippets: Array<string> = [];
  const missingSnippets: Array<string> = [];

  for (const label of referencedLabels) {
    const snippetResult = getSnippet(label);
    if (snippetResult.success) {
      existingSnippets.push(label);
    } else {
      missingSnippets.push(label);
    }
  }

  return {
    hasSnippets,
    referencedLabels,
    existingSnippets,
    missingSnippets,
  };
}
