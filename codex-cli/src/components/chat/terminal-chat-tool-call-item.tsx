import { parseApplyPatch } from "../../parse-apply-patch";
import chalk from "chalk";
import { Text } from "ink";
import React from "react";

export function TerminalChatToolCallCommand({
  commandForDisplay,
  explanation,
}: {
  commandForDisplay: string;
  explanation?: string;
}): React.ReactElement {
  // -------------------------------------------------------------------------
  // Colorize diff output inside the command preview: we detect individual
  // lines that begin with '+' or '-' (excluding the typical diff headers like
  // '+++', '---', '++', '--') and apply green/red coloring.  This mirrors
  // how Git shows diffs and makes the patch easier to review.
  // -------------------------------------------------------------------------

  const colorizedCommand = commandForDisplay
    .split("\n")
    .map((line) => {
      if (line.startsWith("+") && !line.startsWith("++")) {
        return chalk.green(line);
      }
      if (line.startsWith("-") && !line.startsWith("--")) {
        return chalk.red(line);
      }
      return line;
    })
    .join("\n");

  return (
    <>
      <Text bold color="green">
        Shell Command
      </Text>
      <Text>
        <Text dimColor>$</Text> {colorizedCommand}
      </Text>
      {explanation && (
        <>
          <Text bold color="yellow">
            Explanation
          </Text>
          {explanation.split("\n").map((line, i) => {
            // Apply different styling to headings (numbered items)
            if (line.match(/^\d+\.\s+/)) {
              return (
                <Text key={i} bold color="cyan">
                  {line}
                </Text>
              );
            } else if (line.match(/^\s*\*\s+/)) {
              // Style bullet points
              return (
                <Text key={i} color="magenta">
                  {line}
                </Text>
              );
            } else if (line.match(/^(WARNING|CAUTION|NOTE):/i)) {
              // Style warnings
              return (
                <Text key={i} bold color="red">
                  {line}
                </Text>
              );
            } else {
              return <Text key={i}>{line}</Text>;
            }
          })}
        </>
      )}
    </>
  );
}

export function TerminalChatToolCallApplyPatch({
  commandForDisplay,
  patch,
}: {
  commandForDisplay: string;
  patch: string;
}): React.ReactElement {
  const ops = React.useMemo(() => parseApplyPatch(patch), [patch]);
  // Use firstOp for empty-patch detection
  const firstOp = ops?.[0];

  if (ops == null) {
    return (
      <>
        <Text bold color="red">
          Invalid Patch
        </Text>
        <Text color="red" dimColor>
          The provided patch command is invalid.
        </Text>
        <Text dimColor>{commandForDisplay}</Text>
      </>
    );
  }

  if (!firstOp) {
    return (
      <>
        <Text bold color="yellow">
          Empty Patch
        </Text>
        <Text color="yellow" dimColor>
          No operations found in the patch command.
        </Text>
        <Text dimColor>{commandForDisplay}</Text>
      </>
    );
  }

  // Display a human-readable plan: list file operations before showing the patch command
  return (
    <>
      <Text bold color="cyan">
        Plan:
      </Text>
      {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (ops || []).map((op: any, i) => {
          // op is a parsed patch operation: create, delete, or update
          let desc: string;
          if (op.type === "create") {
            desc = `Add file ${op.path}`;
          } else if (op.type === "delete") {
            desc = `Delete file ${op.path}`;
          } else if (op.type === "update") {
            desc = `Update file ${op.path} (+${op.added}/-${op.deleted} lines)`;
          } else {
            desc = `${capitalize(op.type)} ${op.path}`;
          }
          return <Text key={i}>{desc}</Text>;
        })
      }
      <Text>
        <Text dimColor>$</Text> {commandForDisplay}
      </Text>
    </>
  );
}

const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
