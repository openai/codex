# Direct Command Execution in Codex: A Comprehensive Guide

## Introduction

Direct Command Execution is a powerful feature in Codex that allows you to execute shell commands directly without requiring AI processing. This feature provides two distinct command prefixes (`!` and `$`) that offer fine-grained control over how command execution interacts with your AI session context.

## Command Prefixes: `!` vs `$`

### The `!` Command Prefix
- **Purpose**: Execute commands without adding their output to the AI's context
- **Syntax**: `!your-command-here`
- **Example**: `!ls -la`
- **Result**: The command runs and you see the output, but the AI has no knowledge of this command or its results

### The `$` Command Prefix
- **Purpose**: Execute commands AND add their output to the AI's context
- **Syntax**: `$your-command-here`
- **Example**: `$cat package.json`
- **Result**: The command runs, you see the output, AND the AI has access to both the command and its output in future interactions

## Configuration Options

Direct commands can be configured through your `~/.codex/config.json` file:

```json
{
  "directCommands": {
    "autoApprove": true,
    "addToContext": true
  }
}
```

- **autoApprove**: When set to `true`, commands execute without requiring confirmation
- **addToContext**: When set to `true`, `$` prefixed commands add output to context (this is a secondary gate)

## Use Cases and Examples

### When to Use the `!` Prefix

Use `!` for commands that:
- Are for your reference only
- Would pollute the AI context with irrelevant information
- Contain sensitive information you don't want shared with the AI
- Are used frequently for system checks

**Examples:**
```
!pwd                    # Check current directory
!ls -la                 # List files without cluttering context
!git status             # Check git status
!ps aux | grep node     # Check running processes
!npm run test           # Run tests without sharing results
!docker ps              # Check running containers
```

### When to Use the `$` Prefix

Use `$` for commands that:
- Produce output relevant to your current conversation with the AI
- Generate information you want the AI to analyze or reference
- Set up context for your next questions

**Examples:**
```
$cat config.json           # Share configuration file content
$git diff                  # Show code changes for AI review
$npm list --depth=0        # Show dependencies for context
$grep -r "function" src/   # Find and share code patterns
$curl -s api.example.com   # Share API responses for analysis
```

## Practical Workflow Examples

### Workflow 1: Surgical Context Management
```
!git status                           # Check status without adding to context
$git diff src/components/Button.tsx   # Share only relevant file changes
What's wrong with my Button component implementation?
```

### Workflow 2: Keeping Secrets While Getting Help
```
!env | grep API_KEY                  # Check your API keys privately
$grep -r "api.connect" src/ --include="*.js"   # Share only API usage patterns
How should I structure my API calls for better security?
```

### Workflow 3: Test-Driven Development
```
!npm run test                        # Run tests privately to see what's failing
$cat src/utils/validation.js         # Share the implementation
$cat tests/utils/validation.test.js  # Share the failing test
Why is my validation test failing?
```

### Workflow 4: Simple Context Test
This workflow demonstrates how context is managed:
```
!echo "This is a secret that should NOT be in context"
$echo "This information SHOULD be in context"
What information did I just share with you in my commands?
```
The AI will only know about "This information SHOULD be in context" and will have no knowledge of the "secret".

## Best Practices

1. **Start with `!` by default**: Only use `$` when you specifically want information in context
2. **Be context-conscious**: Remember that `$` commands count against your context limit
3. **Use `$` sparingly**: For large outputs, consider filtering or using tools like `head`/`tail` 
4. **Keep sensitive information behind `!`**: Never use `$` with commands that expose secrets
5. **Chain commands thoughtfully**: Use `$` for final output but `!` for intermediate steps
   ```
   !cd src/components
   $ls -la   # Only this output is added to context
   ```
6. **Curate context**: Use `$` only for the most relevant information

## Advanced Usage

### Combining with Pipes and Filters
```
$git log -n 5 --oneline   # Show recent commits but limit output
$grep -A 5 -B 5 "bug" server.log   # Show error with context, but not whole log
```

### Using with Configuration Files
```
!ls ~/.codex          # Check what config files exist
$cat ~/.codex/config.json | jq .   # Pretty-print and share your config
```

### Stateful Operations
Remember that command state is preserved between commands, so you can:
```
!cd src/components
!pwd                   # Confirms you're in the right directory
$ls -la                # Lists the contents of the components directory
```

## Troubleshooting

### Command Not Recognized as Direct Command
- Ensure there's no space after the prefix: Use `!pwd` not `! pwd`
- Make sure your Codex is updated to the version supporting this feature

### AutoApproval Not Working
- Check your config file with `!cat ~/.codex/config.json`
- Ensure `"autoApprove": true` is set correctly
- Try restarting Codex if config changes aren't taking effect

### Output Not Added to Context
- Verify you used `$` and not `!` prefix
- Check your config has `"addToContext": true`
- Be aware of context limits; very large outputs might be truncated

## Conclusion

Direct command execution bridges the gap between your terminal and AI assistant, giving you fine-grained control over what information is shared with the AI. By understanding when to use `!` versus `$`, you can maintain context efficiency, protect sensitive information, and optimize your workflow.

The power of this feature lies in its simplicity: two characters that fundamentally change how you interact with both your system and your AI assistant. Use it wisely to make your Codex experience more productive and secure.