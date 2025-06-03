import {
  addSnippet,
  getSnippet,
  listSnippets,
  removeSnippet,
  type Snippet,
} from "./snippet-storage.js";

export interface SnippetCommandResult {
  success: boolean;
  message: string;
  displayContent?: string;
}

/**
 * Parse snippet command arguments
 */
function parseSnippetArgs(input: string): {
  command: string;
  args: Array<string>;
} {
  const parts = input.trim().split(/\s+/);
  const command = parts[1] || "list"; // Default to list if no subcommand
  const args = parts.slice(2);
  return { command, args };
}

/**
 * Format snippet for display
 */
function formatSnippetDisplay(snippet: Snippet): string {
  const date = new Date(snippet.created_at).toLocaleDateString();
  return `**Snippet: ${snippet.label}** (created: ${date})

\`\`\`
${snippet.code}
\`\`\``;
}

/**
 * Format snippets list for display
 */
function formatSnippetsList(snippets: Array<Snippet>): string {
  if (snippets.length === 0) {
    return "No snippets found. Use `snippet add <label> <code>` to create your first snippet.";
  }

  let output = `**Found ${snippets.length} snippet${snippets.length === 1 ? "" : "s"}:**\n\n`;

  snippets.forEach((snippet, index) => {
    const date = new Date(snippet.created_at).toLocaleDateString();
    const preview =
      snippet.code.length > 50
        ? snippet.code.substring(0, 50) + "..."
        : snippet.code;
    output += `${index + 1}. **${snippet.label}** (${date})\n   ${preview.replace(/\n/g, " ")}\n\n`;
  });

  output += "Use `snippet show <label>` to view a full snippet.";

  return output;
}

/**
 * Handle snippet add command
 */
function handleAddCommand(args: Array<string>): SnippetCommandResult {
  if (args.length < 2) {
    return {
      success: false,
      message:
        'Usage: snippet add <label> <code>\nExample: snippet add debounce "function debounce(fn, delay) { ... }"',
    };
  }

  const label = args[0];
  if (!label) {
    return {
      success: false,
      message: "Label is required",
    };
  }

  const code = args.slice(1).join(" ");

  // Remove surrounding quotes if present
  const cleanCode = code.replace(/^["']|["']$/g, "");

  const result = addSnippet(label, cleanCode);

  return {
    success: result.success,
    message:
      result.message ||
      (result.success ? `Added snippet "${label}"` : "Failed to add snippet"),
  };
}

/**
 * Handle snippet show command
 */
function handleShowCommand(args: Array<string>): SnippetCommandResult {
  if (args.length === 0) {
    return {
      success: false,
      message: "Usage: snippet show <label>\nExample: snippet show debounce",
    };
  }

  const label = args[0];
  if (!label) {
    return {
      success: false,
      message: "Label is required",
    };
  }

  const result = getSnippet(label);

  if (result.success && result.snippet) {
    return {
      success: true,
      message: `Retrieved snippet "${label}"`,
      displayContent: formatSnippetDisplay(result.snippet),
    };
  } else {
    return {
      success: false,
      message: result.message || `Snippet "${label}" not found`,
    };
  }
}

/**
 * Handle snippet list command
 */
function handleListCommand(): SnippetCommandResult {
  const result = listSnippets();

  if (result.success && result.snippets) {
    return {
      success: true,
      message: "Listed all snippets",
      displayContent: formatSnippetsList(result.snippets),
    };
  } else {
    return {
      success: false,
      message: "Failed to load snippets",
    };
  }
}

/**
 * Handle snippet remove command
 */
function handleRemoveCommand(args: Array<string>): SnippetCommandResult {
  if (args.length === 0) {
    return {
      success: false,
      message:
        "Usage: snippet remove <label>\nExample: snippet remove debounce",
    };
  }

  const label = args[0];
  if (!label) {
    return {
      success: false,
      message: "Label is required",
    };
  }

  const result = removeSnippet(label);

  return {
    success: result.success,
    message:
      result.message ||
      (result.success
        ? `Removed snippet "${label}"`
        : "Failed to remove snippet"),
  };
}

/**
 * Handle snippet help command
 */
function handleHelpCommand(): SnippetCommandResult {
  const helpText = `**Snippet Management Commands:**

\`snippet add <label> <code>\` - Add or update a snippet
\`snippet show <label>\` - Display a specific snippet
\`snippet list\` - List all snippets
\`snippet remove <label>\` - Remove a snippet
\`snippet help\` - Show this help

**Examples:**
\`snippet add debounce "function debounce(fn, delay) { let timer; return (...args) => { clearTimeout(timer); timer = setTimeout(() => fn(...args), delay); }; }"\`
\`snippet show debounce\`
\`snippet list\`
\`snippet remove debounce\`

**Notes:**
- Labels must contain only letters, numbers, underscores, and hyphens
- Snippets are stored in ~/.codex/snippets.json
- Use quotes around code containing spaces`;

  return {
    success: true,
    message: "Showed snippet help",
    displayContent: helpText,
  };
}

/**
 * Main snippet command handler
 */
export function handleSnippetCommand(input: string): SnippetCommandResult {
  const { command, args } = parseSnippetArgs(input);

  switch (command.toLowerCase()) {
    case "add":
      return handleAddCommand(args);

    case "show":
      return handleShowCommand(args);

    case "list":
    case "ls":
      return handleListCommand();

    case "remove":
    case "rm":
    case "delete":
      return handleRemoveCommand(args);

    case "help":
    case "--help":
    case "-h":
      return handleHelpCommand();

    default:
      return {
        success: false,
        message: `Unknown snippet command: ${command}\nUse 'snippet help' for available commands`,
      };
  }
}
