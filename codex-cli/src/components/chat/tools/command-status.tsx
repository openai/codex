import { ANIMATION_CYCLE_MS } from "../animation-config";
import { Box, Text } from "ink";
import React, { type ReactElement, useEffect, useState } from "react";

export function CommandStatus({
  title,
  workdir,
  outputText,
  fullStdout,
  marginTop,
}: {
  title: string;
  workdir?: string;
  outputText?: string;
  fullStdout?: boolean;
  marginTop?: number;
}): ReactElement {
  const { label, tail, color, suppressOutput } = splitLabelTailAndColor(title);

  // Animated cursor for running states – replaces the static hourglass.
  const CURSOR_FRAMES = ["·", "•", "●", "•"] as const;
  const isRunningLabel =
    /^(?:⏳|●)\s+(Running|Searching|Listing|Reading)\b/u.test(label);
  const [frameIdx, setFrameIdx] = useState(0);
  useEffect(() => {
    if (!isRunningLabel) {
      return;
    }
    const frameMs = Math.max(
      16,
      Math.round(ANIMATION_CYCLE_MS / CURSOR_FRAMES.length),
    );
    const id = setInterval(
      () => setFrameIdx((i) => (i + 1) % CURSOR_FRAMES.length),
      frameMs,
    );
    return () => clearInterval(id);
  }, [isRunningLabel]);
  const animatedCursor = CURSOR_FRAMES[frameIdx];
  const labelSansIcon = isRunningLabel
    ? label.replace(/^(?:⏳|●)\s+/, "")
    : label;

  const startsWithFailureX = /^⨯\s+/u.test(label);
  return (
    <Box
      flexDirection="column"
      gap={1}
      marginTop={typeof marginTop === "number" ? marginTop : 1}
    >
      <Text>
        {isRunningLabel ? (
          <Text color={color} bold>
            {animatedCursor} {labelSansIcon}
          </Text>
        ) : startsWithFailureX ? (
          <>
            <Text color="red" bold>
              ⨯
            </Text>
            <Text> </Text>
            <Text color="white" bold>
              {label.replace(/^⨯\s+/u, "")}
            </Text>
          </>
        ) : (
          <Text color={color} bold>
            {label}
          </Text>
        )}
        {/* Tail with special formatting for "[Ctrl J to inspect]" */}
        {(() => {
          if (!tail) return <Text dimColor>{tail}</Text>;
          const HINT = "[Ctrl J to inspect]";
          const idx = tail.indexOf(HINT);
          if (idx === -1) {
            return <Text dimColor>{tail}</Text>;
          }
          const before = tail.slice(0, idx);
          const after = tail.slice(idx + HINT.length);
          return (
            <>
              <Text dimColor>{before} [</Text>
              <Text dimColor bold>
                Ctrl J
              </Text>
              <Text dimColor> to inspect]</Text>
              <Text dimColor>{after}</Text>
            </>
          );
        })()}
        {workdir ? <Text dimColor>{` (${workdir})`}</Text> : null}
      </Text>
      {outputText && !suppressOutput ? (
        <Text dimColor>{truncateOutput(outputText, Boolean(fullStdout))}</Text>
      ) : null}
    </Box>
  );
}

function truncateOutput(text: string, fullStdout: boolean | undefined): string {
  if (fullStdout) {
    return text;
  }
  const lines = text.split("\n");
  if (lines.length <= 4) {
    return text;
  }
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
    tailOverride?: string;
  }> = [
    { re: /^((?:⏳|●)\s+Running)(.*)$/u, color: "white" },
    {
      re: /^((?:⏳|●)\s+Searching)(.*)$/u,
      color: "white",
      suppressOutput: true,
    },
    {
      re: /^((?:⏳|●)\s+Listing)(.*)$/u,
      color: "white",
      suppressOutput: true,
    },
    { re: /^((?:⏳|●)\s+Reading)(.*)$/u, color: "white" },
    { re: /^(●\s+Ran)(.*)$/u, color: "white" },
    { re: /^(●\s+Listed)(.*)$/u, color: "white", suppressOutput: true },
    { re: /^(●\s+Counted)(.*)$/u, color: "white", suppressOutput: true },
    { re: /^(●\s+Counted)(.*)$/u, color: "white", suppressOutput: true },
    { re: /^(●\s+Found)(.*)$/u, color: "white", suppressOutput: true },
    {
      re: /^((?:🔍|𓁹)\s+Searched(?:\s+for)?)(.*)$/u,
      color: "white",
      suppressOutput: true,
    },
    { re: /^(●\s+Read)(.*)$/u, color: "white", suppressOutput: true },
    { re: /^(✓\s+Tests)(.*)$/u, color: "white", suppressOutput: false },
    // Failures: render '⨯' in red, rest white, suppress output
    {
      re: /^(⨯\s+Tests failed)(.*)$/u,
      color: "white",
      suppressOutput: true,
      tailOverride: " [Ctrl J to inspect]",
    },
    { re: /^(⨯\s+Failed)(.*)$/u, color: "white", suppressOutput: true },
    {
      re: /^(⨯\s+Command not found)(.*)$/u,
      color: "white",
      suppressOutput: true,
    },
    { re: /^(⨯\s+Aborted)(.*)$/u, color: "white", suppressOutput: true },
  ];
  for (const { re, color, suppressOutput } of patterns) {
    const m = full.match(re);
    if (m) {
      // Special-case added tail for tests failed hint
      const tailExtraMatch = patterns.find(
        (p) => p.re.source === re.source,
      )?.tailOverride;
      return {
        label: m[1] ?? full,
        tail: (m[2] ?? "") + (tailExtraMatch ?? ""),
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
