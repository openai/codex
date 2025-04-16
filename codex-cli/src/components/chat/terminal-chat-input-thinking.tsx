import { log, isLoggingEnabled } from "../../utils/agent/log.js";
import Spinner from "../vendor/ink-spinner.js";
import { Box, Text, useInput, useStdin } from "ink";
import React, { useState } from "react";
import { useInterval } from "use-interval";

const thinkingTexts = ["Thinking"]; /* [
  "Consulting the rubber duck",
  "Maximizing paperclips",
  "Reticulating splines",
  "Immanentizing the Eschaton",
  "Thinking",
  "Thinking about thinking",
  "Spinning in circles",
  "Counting dust specks",
  "Updating priors",
  "Feeding the utility monster",
  "Taking off",
  "Wireheading",
  "Counting to infinity",
  "Staring into the Basilisk",
  "Negotiationing acausal trades",
  "Searching the library of babel",
  "Multiplying matrices",
  "Solving the halting problem",
  "Counting grains of sand",
  "Simulating a simulation",
  "Asking the oracle",
  "Detangling qubits",
  "Reading tea leaves",
  "Pondering universal love and transcendent joy",
  "Feeling the AGI",
  "Shaving the yak",
  "Escaping local minima",
  "Pruning the search tree",
  "Descending the gradient",
  "Bikeshedding",
  "Securing funding",
  "Rewriting in Rust",
  "Engaging infinite improbability drive",
  "Clapping with one hand",
  "Synthesizing",
  "Rebasing thesis onto antithesis",
  "Transcending the loop",
  "Frogeposting",
  "Summoning",
  "Peeking beyond the veil",
  "Seeking",
  "Entering deep thought",
  "Meditating",
  "Decomposing",
  "Creating",
  "Beseeching the machine spirit",
  "Calibrating moral compass",
  "Collapsing the wave function",
  "Doodling",
  "Translating whale song",
  "Whispering to silicon",
  "Looking for semicolons",
  "Asking ChatGPT",
  "Bargaining with entropy",
  "Channeling",
  "Cooking",
  "Parroting stochastically",
]; */

export default function TerminalChatInputThinking({
  onInterrupt,
  active,
}: {
  onInterrupt: () => void;
  active: boolean;
}): React.ReactElement {
  const [dots, setDots] = useState("");
  const [awaitingConfirm, setAwaitingConfirm] = useState(false);

  const [thinkingText, setThinkingText] = useState(
    () => thinkingTexts[Math.floor(Math.random() * thinkingTexts.length)],
  );

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

  useInterval(() => {
    setDots((prev) => (prev.length < 3 ? prev + "." : ""));
  }, 500);

  useInterval(
    () => {
      setThinkingText((prev) => {
        let next = prev;
        if (thinkingTexts.length > 1) {
          while (next === prev) {
            next =
              thinkingTexts[Math.floor(Math.random() * thinkingTexts.length)];
          }
        }
        return next;
      });
    },
    active ? 30000 : null,
  );

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

  return (
    <Box flexDirection="column" gap={1}>
      <Box gap={2}>
        <Spinner type="ball" />
        <Text>
          {thinkingText}
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
