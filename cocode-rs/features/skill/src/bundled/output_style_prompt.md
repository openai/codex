<command-name>/output-style</command-name>

# Output Style Command

You are handling the `/output-style` command. This command manages output style preferences that control how you respond.

## Available Actions

Parse the arguments to determine the action:
- No arguments or `status`: Show current output style status
- `list`: List all available output styles
- `help`: Show help information
- `<style-name>`: Set the specified output style (e.g., `explanatory`, `learning`)
- `off` or `none` or `disable`: Disable output style

## Current Configuration

Check the current output style configuration in the session/config. Report:
- Whether output style is enabled
- Current style name (if set)
- Custom instruction (if set)

## Built-in Styles

The following built-in styles are available:
1. **explanatory** - Provides educational insights while completing tasks
2. **learning** - Hands-on learning with TODO(human) contributions for meaningful design decisions

## Response Format

### For `status` (default):
```
Output Style Status:
  Enabled: [yes/no]
  Current Style: [style name or "none"]
  Custom Instruction: [first 50 chars... or "none"]

Use `/output-style list` to see available styles.
Use `/output-style <name>` to set a style.
```

### For `list`:
```
Available Output Styles:

Built-in:
  - explanatory: Provides educational insights while completing tasks
  - learning: Hands-on learning with TODO(human) contributions

Custom (~/.cocode/output-styles/):
  - [name]: [description from frontmatter]

Use `/output-style <name>` to set a style.
Use `/output-style off` to disable.
```

### For `<style-name>`:
If the style exists, confirm:
```
Output style set to: [style-name]

[Brief description of what this style does]
```

If the style doesn't exist:
```
Unknown style: [style-name]

Available styles: explanatory, learning
Use `/output-style list` to see all available styles.
```

### For `help`:
```
/output-style - Manage response output styles

Usage:
  /output-style              Show current style status
  /output-style list         List available styles
  /output-style <name>       Set output style (e.g., explanatory, learning)
  /output-style off          Disable output style
  /output-style help         Show this help

Styles modify how I respond - adding educational insights, learning opportunities, etc.
```

## Important Notes

- Output style changes take effect for subsequent responses
- Custom instruction (if configured) takes precedence over style name
- Setting a style will update the session configuration
- The actual style content is injected via system reminders
