import type React from "react";

import { render } from "ink-testing-library";
import stripAnsi from "strip-ansi";

/**
 * Render an Ink component for testing.
 *
 * Returns the full testing‑library utils plus `lastFrameStripped()` which
 * yields the latest rendered frame with ANSI escape codes removed so that
 * assertions can be colour‑agnostic.
 */
export function renderTui(ui: React.ReactElement): any {
  const utils = render(ui);

  const lastFrameStripped = () => stripAnsi(utils.lastFrame() || "");

  // A tiny helper that waits for Ink's internal promises / timers to settle
  // so the next `lastFrame()` call reflects the latest UI state.
  const flush = async () =>
    new Promise<void>((resolve) => setTimeout(resolve, 0));

  return {
    ...utils,
    lastFrameStripped,
    flush,
  };
}
