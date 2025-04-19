import { log, isLoggingEnabled } from "../../utils/agent/log.js";
import { Box, Text, useInput, useStdin } from "ink";
import React, { useState } from "react";
import { useInterval } from "use-interval";

// Retaining a single static placeholder text for potential future use.  The
// more elaborate randomised thinking prompts were removed to streamline the
// UI – the elapsed‑time counter now provides sufficient feedback.

export default function TerminalChatInputThinking({
  onInterrupt,
  active,
  thinkingSeconds,
}: {
  onInterrupt: () => void;
  active: boolean;
  thinkingSeconds: number;
}): React.ReactElement {
  const [awaitingConfirm, setAwaitingConfirm] = useState(false);
  const [dots, setDots] = useState("");

  // Animate the ellipsis
  useInterval(() => {
    setDots((prev) => (prev.length < 3 ? prev + "." : ""));
  }, 500);

  const { stdin, setRawMode } = useStdin();

  React.useEffect(() => {
    if (!active) {
      return;
    }

    setRawMode?.(true);

    const onData = (data: Buffer | string) => {
      if (awaitingConfirm) {
        return;
      }

      const str = Buffer.isBuffer(data) ? data.toString("utf8") : data;
      if (str === "\x1b\x1b") {
        if (isLoggingEnabled()) {
          log(
            "raw stdin: received collapsed ESC ESC – starting confirmation timer",
          );
        }
        setAwaitingConfirm(true);
        setTimeout(() => setAwaitingConfirm(false), 1500);
      }
    };

    stdin?.on("data", onData);
    return () => {
      stdin?.off("data", onData);
    };
  }, [stdin, awaitingConfirm, onInterrupt, active, setRawMode]);

  // No timers required beyond tracking the elapsed seconds supplied via props.

  useInput(
    (_input, key) => {
      if (!key.escape) {
        return;
      }

      if (awaitingConfirm) {
        if (isLoggingEnabled()) {
          log("useInput: second ESC detected – triggering onInterrupt()");
        }
        onInterrupt();
        setAwaitingConfirm(false);
      } else {
        if (isLoggingEnabled()) {
          log("useInput: first ESC detected – waiting for confirmation");
        }
        setAwaitingConfirm(true);
        setTimeout(() => setAwaitingConfirm(false), 1500);
      }
    },
    { isActive: active },
  );

  // Custom ball animation including the elapsed seconds
  const ballFrames = [
    "( ●    )",
    "(  ●   )",
    "(   ●  )",
    "(    ● )",
    "(     ●)",
    "(    ● )",
    "(   ●  )",
    "(  ●   )",
    "( ●    )",
    "(●     )",
  ];

  const [frame, setFrame] = useState(0);

  useInterval(() => {
    setFrame((idx) => (idx + 1) % ballFrames.length);
  }, 80);

  // Preserve the spinner (ball) animation while keeping the elapsed seconds
  // text static.  We achieve this by rendering the bouncing ball inside the
  // parentheses and appending the seconds counter *after* the spinner rather
  // than injecting it directly next to the ball (which caused the counter to
  // move horizontally together with the ball).

  const frameTemplate = ballFrames[frame] ?? ballFrames[0];
  const frameWithSeconds = `${frameTemplate} ${thinkingSeconds}s`;

  return (
    <Box flexDirection="column" gap={1}>
      <Box gap={2}>
        <Text>{frameWithSeconds}</Text>
        <Text>
          Thinking
          {dots}
        </Text>
      </Box>
      {awaitingConfirm && (
        <Text dimColor>
          Press <Text bold>Esc</Text> again to interrupt and enter a new
          instruction
        </Text>
      )}
    </Box>
  );
}
