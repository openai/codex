# ansi-escape Package Summary

## Purpose
ANSI escape sequence processing for terminal output formatting. Converts ANSI-formatted text to structured format suitable for terminal UI rendering.

## Key Components

### ANSI Parser
- **Escape Sequence Parsing**: Decode ANSI codes
- **Color Code Handling**: Process color sequences
- **Style Attributes**: Bold, italic, underline
- **Cursor Control**: Handle cursor sequences

### Conversion Engine
- **ANSI to TUI**: Convert to ratatui format
- **Text Segmentation**: Split by formatting
- **Style Preservation**: Maintain formatting
- **Error Recovery**: Handle malformed sequences

### Format Support
- **Color Codes**: 8/16/256/RGB colors
- **Text Styles**: Bold, italic, underline, etc.
- **Cursor Movement**: Position control
- **Clear Operations**: Screen/line clearing

## Main Functionality
1. **ANSI Parsing**: Decode escape sequences
2. **Format Conversion**: Transform to TUI format
3. **Style Application**: Apply text styles
4. **Error Handling**: Graceful degradation
5. **Performance**: Efficient processing

## Dependencies
- `ansi-to-tui`: Core conversion library
- `ratatui`: Terminal UI text types
- String processing utilities

## Integration Points
- Used by `tui` for text rendering
- Processes AI model outputs
- Handles external command output
- Formats log messages

## ANSI Support

### Color Support
- **Basic Colors**: 8 standard colors
- **Bright Colors**: 8 bright variants
- **256 Colors**: Extended palette
- **RGB Colors**: True color support
- **Background Colors**: Bg color support

### Style Attributes
- Bold/bright text
- Dim/faint text
- Italic text
- Underline text
- Blinking text
- Reverse video
- Hidden text
- Strikethrough

### Control Sequences
- Cursor positioning
- Line clearing
- Screen clearing
- Scrolling
- Save/restore cursor

## Use Cases
- **Command Output**: Display colored output
- **Syntax Highlighting**: Show highlighted code
- **Progress Bars**: Render progress indicators
- **Log Formatting**: Colorized logs
- **Error Messages**: Formatted errors

## Performance Features
- **Streaming Processing**: Handle large texts
- **Minimal Allocations**: Memory efficiency
- **Fast Parsing**: Optimized algorithms
- **Lazy Evaluation**: Process on demand

## Error Handling
- **Malformed Sequences**: Skip invalid codes
- **Incomplete Sequences**: Buffer management
- **Unknown Codes**: Ignore unsupported
- **Fallback Rendering**: Plain text fallback

## Text Processing

### Input Handling
- Streaming input
- Buffered input
- Line-by-line
- Chunk processing

### Output Generation
- Structured spans
- Style metadata
- Position information
- Clean text extraction

## Compatibility
- VT100 sequences
- xterm extensions
- Modern terminal codes
- Windows terminal support
- Cross-platform operation