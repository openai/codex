// @ts-nocheck
import React, { useEffect, useState } from "react";
import { Box, Text, useInput } from "ink";
import type { AgentEvent } from "../utils/agent/multi-agent.js";
import type { MultiAgentCoordinator } from "../utils/agent/multi-agent.js";

export default function MultiAgentDashboard({
  coordinator,
}: {
  coordinator: MultiAgentCoordinator;
}): React.ReactElement {
  const [lines, setLines] = useState<Array<{ from: string; text: string }>>([]);

  useEffect(() => {
    const unsub = coordinator.onEvent((e: AgentEvent) => {
      if (e.item.type === "message") {
        const txt = (e.item.content as Array<any> | undefined)
          ?.map((c) => {
            if (typeof c === "string") return c;
            if (c?.text) return c.text;
            if (c?.refusal) return c.refusal;
            return "";
          })
          .join(" ") ?? "";
        setLines((prev: Array<{ from: string; text: string }>) => [
          ...prev.slice(-50),
          { from: e.from, text: txt },
        ]);
      }
    });
    return () => unsub();
  }, [coordinator]);

  // Allow ESC / Ctrl‑C to terminate all agents and exit
  useInput((_input: string, key: { escape: boolean; ctrl: boolean; c: boolean }) => {
    if (key.escape || (key.ctrl && key.c)) {
      coordinator.terminate();
      process.exit(0);
    }
  });

  return (
    <Box flexDirection="column" gap={0}>
      <Text bold underline>
        Multi‑Agent Dashboard (ESC to quit)
      </Text>
      {lines.map((l: { from: string; text: string }, idx: number) => (
        <Text key={idx}>
          <Text color="cyan">[{l.from}] </Text>
          {l.text}
        </Text>
      ))}
    </Box>
  );
} 