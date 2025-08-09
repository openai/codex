// Utilities for classifying shell commands into human-readable titles

/** Return the portion of the string before the first unquoted pipe character. */
export function extractBeforeFirstUnquotedPipe(input: string): string {
  let inSingle = false;
  let inDouble = false;
  for (let i = 0; i < input.length; i += 1) {
    const ch = input[i];
    if (ch === "'" && !inDouble) {
      inSingle = !inSingle;
    } else if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
    } else if (ch === "|" && !inSingle && !inDouble) {
      return input.slice(0, i).trim();
    }
  }
  return input;
}

/** Count non-empty lines in a string. */
function countLines(s?: string): number {
  return (s ? s.split("\n").filter((l) => l.trim().length > 0) : []).length;
}

/** Build a human-readable success title for known commands. */
export function classifySuccessTitle(
  commandText: string,
  outputText?: string,
): string | undefined {
  const cmd = commandText.trim();
  const beforePipe = extractBeforeFirstUnquotedPipe(cmd);
  const lower = beforePipe.toLowerCase();

  // Tests (vitest / npm test / pnpm test)
  if (/(vitest|\b(pnpm|npm|yarn)\s+(run\s+)?test\b)/.test(lower)) {
    return "✅ Tests";
  }

  // 1) ripgrep listings: rg --files
  if (/\brg\b/.test(lower) && /--files(\b|=)/.test(lower)) {
    const n = countLines(outputText);
    return `📁 Listed ${n} ${n === 1 ? "path" : "paths"}`;
  }

  // 2) ripgrep search: rg [opts] PATTERN [PATH]
  if (/\brg\b/.test(lower) && !/--files(\b|=)/.test(lower)) {
    const patternMatch = beforePipe.match(/rg\s+[^"']*(["'])(.*?)\1/);
    const pattern = patternMatch ? patternMatch[2] : undefined;
    const tokens = beforePipe.replace(/\s+/g, " ").trim().split(" ");
    let path: string | undefined;
    for (let i = tokens.length - 1; i >= 0; i -= 1) {
      const t = tokens[i] ?? "";
      if (t === "rg") {
        break;
      }
      if (t.startsWith("-")) {
        continue;
      }
      if (pattern && (t === `"${pattern}"` || t === `'${pattern}'`)) {
        continue;
      }
      path = t;
      break;
    }
    if (pattern && path) {
      return `🔍 Searched for "${pattern}" in "${path}"`;
    }
    if (pattern) {
      return `🔍 Searched for "${pattern}"`;
    }
  }

  // 3) sed -n '1,200p' FILE  => treat as reading FILE
  if (/\bsed\b/.test(lower) && /-n\b/.test(lower) && /p['"]?\b/.test(lower)) {
    const tokens = beforePipe.replace(/\s+/g, " ").trim().split(" ");
    const last = tokens[tokens.length - 1];
    if (last && !last.startsWith("-") && !/['"]\d+,\d+p['"]/.test(last)) {
      return `📖 Read ${last}`;
    }
  }

  // 4) cat FILE => Read FILE
  if (/^cat\s+/.test(lower)) {
    const m = beforePipe.match(/^cat\s+([^\s|&;]+)/);
    if (m && m[1]) {
      return `📖 Read ${m[1]}`;
    }
  }

  // 5) ls/find directory listings – fallback to listed paths using output count
  if (/^(ls|find)\b/.test(lower)) {
    const n = countLines(outputText);
    if (n > 0) {
      return `📁 Listed ${n} ${n === 1 ? "path" : "paths"}`;
    }
  }

  // 6) Console prints: echo / node -e console.log(...)
  if (/^echo\s+/.test(lower)) {
    return "📣 Printed output";
  }
  if (/\bnode\b\s+-e\b/.test(lower) && /console\.log\s*\(/i.test(cmd)) {
    return "📣 Printed output";
  }

  // 6) Generic counters via wc -l pipeline with numeric output
  if (/\|\s*wc\s+-l\b/.test(cmd) && /^\s*\d+\s*$/.test(outputText ?? "")) {
    const n = Number((outputText ?? "0").trim());
    // Count kinds by upstream command
    if (/\brg\b/.test(lower) && /--files(\b|=)/.test(lower)) {
      return `📁 Counted ${n} ${n === 1 ? "path" : "paths"}`;
    }
    if (/\bfind\b/.test(lower) || /\bls\b/.test(lower)) {
      return `📁 Counted ${n} ${n === 1 ? "entry" : "entries"}`;
    }
    const pat = beforePipe.match(/rg\s+[^"']*(["'])(.*?)\1/);
    if (/\brg\b/.test(lower) && pat) {
      return `🔍 Found ${n} ${n === 1 ? "match" : "matches"}`;
    }
    return `📄 Counted ${n} ${n === 1 ? "line" : "lines"}`;
  }

  return undefined;
}

/** Build a human-readable running title for known commands. */
export function classifyRunningTitle(commandText: string): string | undefined {
  const cmd = commandText.trim();
  const beforePipe = extractBeforeFirstUnquotedPipe(cmd);
  const lower = beforePipe.toLowerCase();

  // Tests (vitest / npm test / pnpm test)
  if (/(vitest|\b(pnpm|npm|yarn)\s+(run\s+)?test\b)/.test(lower)) {
    return "⏳ Running tests";
  }

  // rg --files => Listing
  if (/\brg\b/.test(lower) && /--files(\b|=)/.test(lower)) {
    return `⏳ Listing ${commandText}`;
  }

  // rg pattern path => Searching
  if (/\brg\b/.test(lower) && !/--files(\b|=)/.test(lower)) {
    const patternMatch = beforePipe.match(/rg\s+[^"']*(["'])(.*?)\1/);
    const pattern = patternMatch ? patternMatch[2] : undefined;
    const tokens = beforePipe.replace(/\s+/g, " ").trim().split(" ");
    let path: string | undefined;
    for (let i = tokens.length - 1; i >= 0; i -= 1) {
      const t = tokens[i] ?? "";
      if (t === "rg") {
        break;
      }
      if (t.startsWith("-")) {
        continue;
      }
      if (pattern && (t === `"${pattern}"` || t === `'${pattern}'`)) {
        continue;
      }
      path = t;
      break;
    }
    if (pattern && path) {
      return `⏳ Searching for "${pattern}" in "${path}"`;
    }
    if (pattern) {
      return `⏳ Searching for "${pattern}"`;
    }
    return `⏳ Searching ${commandText}`;
  }

  // sed/cat => Reading
  if (
    (/\bsed\b/.test(lower) && /-n\b/.test(lower) && /p['"]?\b/.test(lower)) ||
    /^cat\s+/.test(lower)
  ) {
    return `⏳ Reading ${commandText}`;
  }

  // ls/find => Listing
  if (/^(ls|find)\b/.test(lower)) {
    return `⏳ Listing ${commandText}`;
  }

  return undefined;
}

/** Build a human-readable failure title for known error modes. */
export function classifyFailureTitle(
  commandText: string,
  outputText?: string,
): string | undefined {
  const cmd = commandText.trim();
  const beforePipe = extractBeforeFirstUnquotedPipe(cmd);
  const lower = beforePipe.toLowerCase();
  const out = (outputText ?? "").toLowerCase();

  // sed: file not found
  if (/\bsed\b/.test(lower) && /no such file or directory/i.test(out)) {
    const tokens = beforePipe.replace(/\s+/g, " ").trim().split(" ");
    const last = tokens[tokens.length - 1];
    if (last && !last.startsWith("-")) {
      return `📄 File not found ${last}`;
    }
    return "📄 File not found";
  }

  // Tests failed
  if (/(vitest|\b(pnpm|npm|yarn)\s+(run\s+)?test\b)/.test(lower)) {
    return "❌ Tests failed";
  }

  // Command not found
  if (/command not found/i.test(out)) {
    const first = beforePipe.split(/\s+/)[0] ?? "command";
    return `⚠️ Command not found ${first}`;
  }

  // Permission denied
  if (/permission denied/i.test(out)) {
    return "🚫 Permission denied";
  }

  return undefined;
}
