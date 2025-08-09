import React, { type ReactElement } from "react";
import { Box, Text } from "ink";

export function CommandStatus({
  title,
  workdir,
  outputText,
  fullStdout,
}: {
  title: string;
  workdir?: string;
  outputText?: string;
  fullStdout?: boolean;
}): ReactElement {
  const { label, tail, color, suppressOutput } = splitLabelTailAndColor(title);

  return (
    <Box flexDirection="column" gap={1} marginTop={1}>
      <Text>
        <Text color={color} bold>
          {label}
        </Text>
        <Text dimColor>{tail}</Text>
        {workdir ? <Text dimColor>{` (${workdir})`}</Text> : null}
      </Text>
      {outputText && !suppressOutput ? (
        <Text dimColor>{truncateOutput(outputText, Boolean(fullStdout))}</Text>
      ) : null}
    </Box>
  );
}

function truncateOutput(text: string, fullStdout: boolean | undefined): string {
  if (fullStdout) return text;
  const lines = text.split("\n");
  if (lines.length <= 4) return text;
  const head = lines.slice(0, 4);
  const remaining = lines.length - 4;
  return [...head, `... (${remaining} more lines)`].join("\n");
}

function splitLabelTailAndColor(full: string): {
  label: string;
  tail: string;
  color: Parameters<typeof Text>[0]["color"];
  suppressOutput: boolean;
} {
  const patterns: Array<{
    re: RegExp;
    color: Parameters<typeof Text>[0]["color"];
    suppressOutput?: boolean;
  }> = [
    { re: /^(⏳\s+Running)(.*)$/u, color: "yellow" },
    { re: /^(⏳\s+Searching)(.*)$/u, color: "yellow", suppressOutput: true },
    { re: /^(⏳\s+Listing)(.*)$/u, color: "yellow", suppressOutput: true },
    { re: /^(⏳\s+Reading)(.*)$/u, color: "yellow" },
    { re: /^(⚡\s+Ran)(.*)$/u, color: "green" },
    { re: /^(📁\s+Listed)(.*)$/u, color: "green", suppressOutput: true },
    { re: /^(📁\s+Counted)(.*)$/u, color: "green", suppressOutput: true },
    { re: /^(📄\s+Counted)(.*)$/u, color: "green", suppressOutput: true },
    { re: /^(🔍\s+Found)(.*)$/u, color: "green", suppressOutput: true },
    {
      re: /^((?:🔍|𓁹)\s+Searched(?:\s+for)?)(.*)$/u,
      color: "green",
      suppressOutput: true,
    },
    { re: /^(📖\s+Read)(.*)$/u, color: "green", suppressOutput: true },
    { re: /^(✅\s+Tests)(.*)$/u, color: "green", suppressOutput: false },
    { re: /^(❌\s+Failed)(.*)$/u, color: "red" },
    { re: /^(⏹️\s+Aborted)(.*)$/u, color: "red" },
  ];
  for (const { re, color, suppressOutput } of patterns) {
    const m = full.match(re);
    if (m) {
      return {
        label: m[1] ?? full,
        tail: m[2] ?? "",
        color,
        suppressOutput: Boolean(suppressOutput),
      };
    }
  }
  return {
    label: full,
    tail: "",
    color: "magentaBright",
    suppressOutput: false,
  };
}
