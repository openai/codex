import { describe, it, expect } from 'vitest';
import { scoreAndSortFiles } from '../src/utils/singlepass/relevance_scoring';

describe('scoreAndSortFiles', () => {
  const filePaths = [
    'src/components/App.tsx',
    'src/utils/context_files.ts',
    'src/utils/relevance_scoring.ts', // Exact match filename
    'README.md',
    'docs/architecture.md',
    'src/styles/main.css',
    'test/scoring.test.ts',
    'src/agent/loop.ts',
    'src/components/Button.tsx',
  ];

  it('should return empty array if no files are provided', () => {
    const result = scoreAndSortFiles([], 'query', 5);
    expect(result).toEqual([]);
  });

  it('should return original file order (up to maxFiles) if query is empty', () => {
    const result = scoreAndSortFiles(filePaths, '', 3);
    expect(result.map(f => f.filePath)).toEqual([
      'src/components/App.tsx',
      'src/utils/context_files.ts',
      'src/utils/relevance_scoring.ts',
    ]);
    expect(result[0]?.reason).toBe('No query');
  });

 it('should return original file order (up to maxFiles) if no meaningful keywords found', () => {
    const result = scoreAndSortFiles(filePaths, ' a is the for ', 3);
    expect(result.map(f => f.filePath)).toEqual([
      'src/components/App.tsx',
      'src/utils/context_files.ts',
      'src/utils/relevance_scoring.ts',
    ]);
     expect(result[0]?.reason).toBe('No keywords');
  });


  it('should prioritize files with keywords in the filename', () => {
    const result = scoreAndSortFiles(filePaths, 'relevance scoring test', 5);
    const sortedPaths = result.map(f => f.filePath);

    // Expect scoring files to be ranked highest
    expect(sortedPaths[0]).toBe('test/scoring.test.ts');
    expect(sortedPaths[1]).toBe('src/utils/relevance_scoring.ts');
  });

  it('should prioritize files with keywords in the path over non-matching files', () => {
    const result = scoreAndSortFiles(filePaths, 'search components', 5);
    const sortedPaths = result.map(f => f.filePath);

    // Expect component files to be ranked higher than unrelated files
    expect(sortedPaths).toContain('src/components/App.tsx');
    expect(sortedPaths).toContain('src/components/Button.tsx');
    expect(sortedPaths.indexOf('src/components/App.tsx')).toBeLessThan(sortedPaths.indexOf('README.md'));
  });

 it('should include lower-scoring files if higher-scoring files are less than maxFiles', () => {
    const result = scoreAndSortFiles(filePaths, 'onlyonerelvantkeyword architecture', 10); // Only docs/architecture.md matches significantly
    expect(result.length).toBe(9);
    expect(result[0]!.filePath).toBe('docs/architecture.md');
    // Ensure other files are included to fill up to maxFiles
    expect(result.map(f => f.filePath)).toContain('README.md');
    expect(result.map(f => f.filePath)).toContain('src/components/App.tsx');
  });

  it('should limit results to maxFiles', () => {
    const result = scoreAndSortFiles(filePaths, 'utils component', 3);
    expect(result.length).toBe(3);
  });
}); 