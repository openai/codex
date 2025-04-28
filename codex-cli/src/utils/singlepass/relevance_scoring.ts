import path from 'path';

export interface ScoredFile {
  filePath: string;
  score: number;
  reason?: string; // For potential telemetry/debugging
}

/**
 * Extracts keywords from a user query.
 * Basic implementation: splits by space and filters common words.
 * TODO: Enhance with more sophisticated NLP techniques.
 */
function extractKeywords(query: string): string[] {
  const commonWords = new Set(['a', 'an', 'the', 'is', 'are', 'this', 'that', 'to', 'in', 'it', 'for', 'of', 'and', 'explain', 'codebase', 'me', 'file', 'files', 'how', 'what', 'why', 'does']);
  return query
    .toLowerCase()
    .split(/[\s\W]+/) // Split by whitespace and non-word characters
    .filter(word => word.length > 2 && !commonWords.has(word));
}

/**
 * Scores a file based on relevance to the query keywords.
 * Basic implementation: scores based on keyword matches in the filename and path.
 * TODO: Enhance with content analysis, file characteristics, etc.
 */
function scoreFile(filePath: string, keywords: string[]): ScoredFile {
  let score = 0;
  const lowerFilePath = filePath.toLowerCase();
  const baseName = path.basename(lowerFilePath);
  const dirName = path.dirname(lowerFilePath);

  for (const keyword of keywords) {
    // Higher score for matches in the filename
    if (baseName.includes(keyword)) {
      score += 10;
    }
    // Lower score for matches in the directory path
    if (dirName.includes(keyword)) {
      score += 2;
    }
  }

  // TODO: Add scoring based on file type, recency, etc.

  // Basic telemetry hook placeholder
  // trackEvent('FileScored', { filePath, score, keywords });

  return { filePath, score };
}

/**
 * Takes a list of file paths and a user query, returns a sorted list
 * of files scored by relevance.
 */
export function scoreAndSortFiles(
  filePaths: string[],
  query: string,
  maxFiles: number // Consider token limits implicitly via maxFiles for now
): ScoredFile[] {
  if (!query?.trim()) {
    // If no query, return first N files (existing behavior?)
    // Or apply default scoring (e.g., prioritize README, recently modified?)
    // For now, just return the first maxFiles without scoring.
    // TODO: Define behavior for empty/missing query
    console.warn('Relevance scoring skipped: No query provided.');
    return filePaths.slice(0, maxFiles).map(filePath => ({ filePath, score: 0, reason: 'No query' }));
  }

  const keywords = extractKeywords(query);
  if (keywords.length === 0) {
      console.warn('Relevance scoring skipped: No meaningful keywords extracted from query.');
      return filePaths.slice(0, maxFiles).map(filePath => ({ filePath, score: 0, reason: 'No keywords' }));
  }


  const scoredFiles = filePaths.map(filePath => scoreFile(filePath, keywords));

  // Sort by score descending
  scoredFiles.sort((a, b) => b.score - a.score);

  // Filter out files with zero score *unless* we don't have enough high-scoring files
  const relevantFiles = scoredFiles.filter(f => f.score > 0);
  const otherFiles = scoredFiles.filter(f => f.score === 0);

  // Combine, prioritizing relevant files, up to maxFiles
  const finalFiles = [...relevantFiles, ...otherFiles].slice(0, maxFiles);

  // Basic telemetry hook placeholder
  // trackEvent('FilesSelectedByRelevance', { query, count: finalFiles.length, keywords });

  return finalFiles;
} 