# apply-patch Package Summary

## Purpose
Tool for applying code patches in a structured format used by Codex AI. Enables AI models to make precise code modifications through a custom patch format that's more reliable than traditional diff formats.

## Key Components

### Patch Parser
- **Custom Format Parser**: Parse Codex-specific patch syntax
- **Block Recognition**: Identify add/delete/update blocks
- **Syntax Validation**: Validate patch format
- **Error Recovery**: Handle malformed patches

### File Operations
- **Add Operations**: Create new files
- **Delete Operations**: Remove files
- **Update Operations**: Modify existing files
- **Batch Processing**: Apply multiple patches

### Tree-sitter Integration
- **Bash Parsing**: Parse bash scripts for modifications
- **Syntax-aware Editing**: Understand code structure
- **AST Manipulation**: Work with abstract syntax trees
- **Language Support**: Extensible to other languages

### Diff Generation
- **Unified Diff**: Generate standard diff output
- **Preview Mode**: Show changes without applying
- **Rollback Support**: Undo applied changes
- **Conflict Detection**: Identify conflicting changes

## Main Functionality
1. **Patch Application**: Apply structured patches to files
2. **Format Parsing**: Parse custom patch format
3. **Validation**: Ensure patch validity
4. **Preview**: Show changes before applying
5. **Error Handling**: Graceful failure recovery

## Dependencies
- `similar`: Text diffing and comparison
- `tree-sitter`: Code parsing
- `tree-sitter-bash`: Bash language support
- File I/O libraries
- Unicode text processing

## Integration Points
- Invoked by AI models during conversations
- Used by `core` for code modifications
- Available as standalone tool
- Integrated via `arg0` dispatcher

## Patch Format

### Structure
```
<<<<<<< ADD path/to/file
new file content
=======

<<<<<<< DELETE path/to/file
=======

<<<<<<< UPDATE path/to/file
old content to replace
=======
new content to insert
>>>>>>>
```

### Operations
- **ADD**: Create new file with content
- **DELETE**: Remove existing file
- **UPDATE**: Replace content in file

### Features
- Line-by-line matching
- Whitespace handling
- Unicode support
- Multi-patch files

## Use Cases
- **AI Code Generation**: Let AI create/modify code
- **Automated Refactoring**: Apply systematic changes
- **Code Templates**: Apply template transformations
- **Batch Updates**: Update multiple files
- **Safe Modifications**: Preview before applying

## Safety Features
- Backup creation
- Dry-run mode
- Validation before apply
- Atomic operations
- Rollback capability

## Error Handling
- Clear error messages
- Partial application prevention
- Recovery suggestions
- Detailed diagnostics

## Performance
- Streaming file processing
- Minimal memory usage
- Efficient diff algorithms
- Parallel file operations (when safe)