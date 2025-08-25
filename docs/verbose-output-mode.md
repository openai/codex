# Verbose Output Mode

This feature addresses issue #2655 by adding configurable verbose output modes to Codex, allowing users to control how much command output is displayed.

## Features

### 1. Configuration Options

Add the following to your `~/.codex/config.toml`:

```toml
[output]
verbosity = "auto"           # Options: summary, auto, verbose, full
max_lines = 100              # Maximum lines to show in verbose mode
truncate_strategy = "middle" # Options: head, tail, middle
auto_expand_errors = true    # Automatically show full output on errors
```

### 2. CLI Flags

```bash
# Set verbose mode via command line
codex --verbose auto
codex -v verbose

# Set maximum output lines
codex --max-output-lines 200
```

### 3. Runtime Commands

During a Codex session, you can use:

- `/verbose [mode]` - Toggle verbose mode or set specific level
  - `/verbose` - Toggle between summary and verbose
  - `/verbose auto` - Set to auto mode
  - `/verbose full` - Show all output
  
- `/expand` - Show full output of the last command
  - `/expand` - Show last command's full output
  - `/expand 2` - Show full output from 2 commands ago

## Verbosity Levels

### Summary (Default)
Shows first and last 5 lines of output for commands with more than 10 lines.

### Auto
Automatically determines verbosity based on:
- Error detection (shows full output if errors are found)
- Important commands (build, test, compile commands get verbose output)
- Output length (short outputs are shown in full)

### Verbose
Shows up to `max_lines` of output, truncating based on `truncate_strategy`.

### Full
Shows complete output without any truncation.

## Truncation Strategies

### Head
Shows the first N lines and indicates how many were omitted.

### Tail
Shows the last N lines and indicates how many previous lines were omitted.

### Middle
Shows the first N/2 and last N/2 lines with omission indicator in between.

## Error Detection

When `auto_expand_errors` is enabled, the system automatically detects error indicators:
- error:, Error:, ERROR:
- failed, Failed, FAILED
- panic:, exception:, fatal:
- warning: (optional)

## Important Commands

The following command patterns are considered important and receive verbose output in auto mode:
- test, build, compile
- npm run, cargo, make
- pytest, jest, mocha, rspec
- go test, mvn, gradle
- yarn, pnpm
- docker, kubectl

## Implementation Details

### Core Components

1. **OutputConfig** (`config_types.rs`)
   - Stores verbosity settings
   - Manages truncation strategies
   - Controls error detection

2. **OutputFormatter** (`output_formatter.rs`)
   - Formats command output based on configuration
   - Implements truncation strategies
   - Detects errors and important commands

3. **OutputBuffer** (`output_formatter.rs`)
   - Stores recent command outputs
   - Enables `/expand` functionality
   - Maintains command history

4. **ExecOutputHandler** (`exec_output_handler.rs`)
   - Integrates formatter with execution system
   - Manages runtime verbosity changes
   - Provides thread-safe access to formatter

### Integration Points

- Config system extended to support output configuration
- CLI parser updated with verbose flags
- Slash commands added for runtime control
- Execution pipeline modified to use formatter

## Examples

### Example 1: Build Command with Error

```bash
$ npm run build
```

With `verbosity = "auto"` and `auto_expand_errors = true`:
- If build succeeds: Shows summary
- If build fails: Shows full error output

### Example 2: Long Test Output

```bash
$ pytest tests/
```

With `verbosity = "verbose"` and `max_lines = 50`:
- Shows first 25 and last 25 lines
- Indicates number of omitted lines

### Example 3: Runtime Toggle

```
User: Run the tests
Codex: Running tests...
[Shows summary output]

User: /expand
Codex: [Shows full test output]

User: /verbose full
Codex: Verbose mode set to: full

User: Run the build
Codex: [Shows complete build output]
```

## Testing

The implementation includes comprehensive tests:

1. **Unit Tests** (`output_formatter.rs`)
   - Test summary formatting
   - Test error detection
   - Test important command detection
   - Test truncation strategies
   - Test output buffer functionality

2. **Integration Tests**
   - Test config loading
   - Test CLI flag parsing
   - Test runtime command handling

## Future Enhancements

Potential improvements for future iterations:

1. **Persistent Settings**
   - Save verbose preferences per project
   - Remember user's preferred verbosity level

2. **Smart Detection**
   - Learn from user behavior
   - Customize important command patterns

3. **Output Filtering**
   - Regex-based filtering
   - Highlight specific patterns

4. **Export Functionality**
   - Save full outputs to file
   - Export command history

## Migration Guide

For users upgrading to the new version:

1. No breaking changes - defaults maintain current behavior
2. Optional configuration in `config.toml`
3. New CLI flags are opt-in
4. Runtime commands available immediately

## Troubleshooting

### Output Not Showing as Expected

1. Check your `~/.codex/config.toml` for output settings
2. Verify CLI flags aren't overriding config
3. Use `/status` to see current verbosity setting

### Performance Issues with Large Outputs

1. Reduce `max_lines` setting
2. Use `truncate_strategy = "tail"` for log files
3. Consider `verbosity = "summary"` for routine commands