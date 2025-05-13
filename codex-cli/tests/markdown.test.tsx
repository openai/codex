import type { ColorSupportLevel } from "chalk";

import { renderTui } from "./ui-test-helpers.js";
import { Markdown } from "../src/components/chat/terminal-chat-response-item.js";
import React from "react";
import { describe, afterEach, beforeEach, it, expect, vi } from "vitest";
import chalk from "chalk";

/** Simple sanity check that the Markdown component renders bold/italic text.
 * We strip ANSI codes, so the output should contain the raw words. */
it("renders basic markdown", () => {
  const { lastFrameStripped } = renderTui(
    <Markdown fileOpener={undefined}>**bold** _italic_</Markdown>,
  );

  const frame = lastFrameStripped();
  expect(frame).toContain("bold");
  expect(frame).toContain("italic");
});

describe("ensure <Markdown> produces content with correct ANSI escape codes", () => {
  let chalkOriginalLevel: ColorSupportLevel = 0;

  beforeEach(() => {
    chalkOriginalLevel = chalk.level;
    chalk.level = 3;

    vi.mock("supports-hyperlinks", () => ({
      default: {},
      supportsHyperlink: () => true,
      stdout: true,
      stderr: true,
    }));
  });

  afterEach(() => {
    vi.resetAllMocks();
    chalk.level = chalkOriginalLevel;
  });

  it("renders basic markdown with ansi", () => {
    const { lastFrame } = renderTui(
      <Markdown fileOpener={undefined}>**bold** _italic_</Markdown>,
    );

    const frame = lastFrame();
    const BOLD = "\x1B[1m";
    const BOLD_OFF = "\x1B[22m";
    const ITALIC = "\x1B[3m";
    const ITALIC_OFF = "\x1B[23m";
    expect(frame).toBe(`${BOLD}bold${BOLD_OFF} ${ITALIC}italic${ITALIC_OFF}`);
  });

  it("citations should get converted to hyperlinks when stdout supports them", () => {
    const { lastFrame } = renderTui(
      <Markdown fileOpener={"vscode"} cwd="/foo/bar">
        File with TODO: 【F:src/approvals.ts†L40】
      </Markdown>,
    );

    const BLUE = "\x1B[34m";
    const LINK_ON = "\x1B[4m";
    const LINK_OFF = "\x1B[24m";
    const COLOR_OFF = "\x1B[39m";

    const expected = `File with TODO: ${BLUE}src/approvals.ts (${LINK_ON}vscode://file/foo/bar/src/approvals.ts:40${LINK_OFF})${COLOR_OFF}`;
    const outputWithAnsi = lastFrame();
    expect(outputWithAnsi).toBe(expected);
  });
});
