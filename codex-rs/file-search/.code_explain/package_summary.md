# file-search Package Summary

## Purpose
Fast file searching and filtering tool with fuzzy matching capabilities. Provides efficient file discovery and navigation for the Codex system with parallel processing and smart filtering.

## Key Components

### Search Engine
- **Parallel Traversal**: Multi-threaded file walking
- **Fuzzy Matching**: Approximate string matching
- **Pattern Recognition**: Multiple search patterns
- **Result Ranking**: Relevance scoring

### File Walking
- **Ignore Integration**: Respect .gitignore rules
- **Custom Filters**: Additional exclusion patterns
- **Depth Control**: Limit search depth
- **Performance Optimization**: Smart caching

### Matching Algorithms
- **Nucleo Matcher**: High-performance fuzzy matching
- **Exact Matching**: Literal string matching
- **Regex Support**: Regular expression patterns
- **Score Calculation**: Match quality scoring

### Result Management
- **Result Limiting**: Configurable result caps
- **Sorting**: Multiple sort strategies
- **Deduplication**: Remove duplicate results
- **Streaming**: Progressive result delivery

## Main Functionality
1. **File Discovery**: Find files in directory trees
2. **Fuzzy Search**: Approximate name matching
3. **Pattern Filtering**: Include/exclude patterns
4. **Parallel Processing**: Multi-core utilization
5. **Smart Ranking**: Relevance-based ordering

## Dependencies
- `ignore`: Gitignore-aware file walking
- `nucleo-matcher`: Fuzzy matching engine
- `tokio`: Async runtime
- `rayon`: Parallel processing
- Path and file utilities

## Integration Points
- Used by `tui` for file browsing
- Integrated in `core` for file operations
- Available as standalone tool
- Powers file navigation features

## Search Features

### Pattern Types
- **Literal**: Exact string matching
- **Fuzzy**: Approximate matching
- **Glob**: Wildcard patterns
- **Regex**: Regular expressions

### Filtering Options
- **File Types**: Filter by extension
- **Size Limits**: Min/max file size
- **Date Filters**: Modified/created times
- **Path Filters**: Include/exclude paths

### Performance Features
- **Parallel Walking**: Use all CPU cores
- **Early Termination**: Stop when limit reached
- **Memory Efficiency**: Stream processing
- **Cache Utilization**: Reuse previous results

## Use Cases
- **IDE-like File Finding**: Quick file navigation
- **Project Exploration**: Discover project structure
- **Code Search**: Find specific files
- **Asset Discovery**: Locate resources
- **Batch Operations**: Find files for processing

## Optimization Strategies
- **Smart Defaults**: Ignore common non-source files
- **Incremental Search**: Refine previous results
- **Index Caching**: Cache directory structures
- **Priority Queuing**: Process likely matches first

## Configuration
- Search depth limits
- Result count limits
- Ignore patterns
- Custom filters
- Scoring weights

## Output Formats
- File paths only
- Paths with scores
- Detailed match info
- JSON structured output
- Streaming results