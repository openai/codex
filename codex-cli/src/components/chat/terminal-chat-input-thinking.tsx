import { log } from "../../utils/logger/log.js";
import { Box, Text, useInput } from "ink";
import React, { useEffect, useMemo, useState } from "react";
import { ANIMATION_CYCLE_MS } from "./animation-config";

// Retaining a single static placeholder text for potential future use.  The
// more elaborate randomised thinking prompts were removed to streamline the
// UI – the elapsed‑time counter now provides sufficient feedback.

export default function TerminalChatInputThinking({
  onInterrupt,
  active,
  thinkingSeconds: _thinkingSeconds,
  title,
}: {
  onInterrupt: () => void;
  active: boolean;
  thinkingSeconds: number;
  title?: string;
}): React.ReactElement {
  const [awaitingConfirm, setAwaitingConfirm] = useState(false);
  const [persistentTitle, setPersistentTitle] = useState<string>("");
  const [phase, setPhase] = useState<number>(0);

  // Avoid forcing raw-mode globally; rely on Ink's useInput handling with isActive

  // No timers required beyond tracking the elapsed seconds supplied via props.

  // Keep last non-empty, non-default (not "Thinking") title visible until a new one arrives
  useEffect(() => {
    const incoming = typeof title === "string" ? title.trim() : "";
    const isDefaultThinking = /^thinking$/i.test(incoming);
    if (
      incoming.length > 0 &&
      !isDefaultThinking &&
      incoming !== persistentTitle
    ) {
      setPersistentTitle(incoming);
      setPhase(0);
    }
  }, [title, persistentTitle]);

  useInput(
    (_input, key) => {
      if (!key.escape) {
        return;
      }

      if (awaitingConfirm) {
        log("useInput: second ESC detected – triggering onInterrupt()");
        onInterrupt();
        setAwaitingConfirm(false);
      } else {
        log("useInput: first ESC detected – waiting for confirmation");
        setAwaitingConfirm(true);
        setTimeout(() => setAwaitingConfirm(false), 1500);
      }
    },
    { isActive: active },
  );

  // Animate a sliding shimmer over the title using levels 0..3 (white→dark gray)
  const animatedNodes = useMemo(() => {
    const text = (persistentTitle || "Thinking").split("");
    const n = text.length;
    // More ranges with darkest in the middle (peak)
    // Intensities 0 (white) .. 8 (darkest)
    const kernel = [
      1,
      2,
      3,
      4,
      5,
      6,
      7,
      8, // ramp up to darkest
      8,
      7,
      6,
      5,
      4,
      3,
      2,
      1, // ramp down symmetrically
    ];
    const levels = new Array<number>(n).fill(0);
    const center = phase % Math.max(1, n); // center position moves across text
    const half = Math.floor(kernel.length / 2);
    for (let k = 0; k < kernel.length; k += 1) {
      const idx = (center - half + k + n) % n; // wrap kernel around text
      levels[idx] = kernel[k] ?? 0;
    }

    // Palette from white (0) to very dark gray (8)
    const palette = [
      "#FFFFFF", // 0
      "#EDEDED", // 1
      "#DBDBDB", // 2
      "#C9C9C9", // 3
      "#B7B7B7", // 4
      "#A5A5A5", // 5
      "#8F8F8F", // 6
      "#6F6F6F", // 7
      "#4A4A4A", // 8 darkest
    ];

    return text.map((ch, i) => {
      const lvl = levels[i] ?? 0;
      const color = palette[Math.max(0, Math.min(palette.length - 1, lvl))];
      return (
        <Text key={i} color={color}>
          {ch}
        </Text>
      );
    });
  }, [persistentTitle, phase]);

  useEffect(() => {
    if (!active) {
      return;
    }
    const textLen = (persistentTitle || "Thinking").length;
    const cycle = Math.max(1, textLen); // number of positions for a full pass
    const frameMs = Math.max(16, Math.round(ANIMATION_CYCLE_MS / cycle));
    const id = setInterval(() => {
      setPhase((p) => (p + 1) % cycle);
    }, frameMs);
    return () => clearInterval(id);
  }, [active, persistentTitle]);

  return (
    <Box flexDirection="column" gap={1}>
      <Box>
        <Text>{animatedNodes}</Text>
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
