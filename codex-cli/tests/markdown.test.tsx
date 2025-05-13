import type { ColorSupportLevel } from "chalk";

import { renderTui } from "./ui-test-helpers.js";
import { Markdown } from "../src/components/chat/terminal-chat-response-item.js";
import React from "react";
import { describe, afterEach, beforeEach, it, expect, vi } from "vitest";
import chalk from "chalk";

const BOLD = "\x1B[1m";
const BOLD_OFF = "\x1B[22m";
const ITALIC = "\x1B[3m";
const ITALIC_OFF = "\x1B[23m";
const LINK_ON = "\x1B[4m";
const LINK_OFF = "\x1B[24m";
const BLUE = "\x1B[34m";
const GREEN = "\x1B[32m";
const YELLOW = "\x1B[33m";
const COLOR_OFF = "\x1B[39m";

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
    expect(frame).toBe(`${BOLD}bold${BOLD_OFF} ${ITALIC}italic${ITALIC_OFF}`);
  });

  // We had to patch in https://github.com/mikaelbr/marked-terminal/pull/366 to
  // make this work.
  it("bold test in a bullet should be rendered correctly", () => {
    const { lastFrame } = renderTui(
      <Markdown fileOpener={undefined}>* **bold** text</Markdown>,
    );

    const outputWithAnsi = lastFrame();
    expect(outputWithAnsi).toBe(`* ${BOLD}bold${BOLD_OFF} text`);
  });

  it("ensure simple nested list works as expected", () => {
    // Empirically, if there is no text at all before the first list item,
    // it gets indented.
    const nestedList = `\
Paragraph before bulleted list.

* item 1
  * subitem 1
  * subitem 2
* item 2
`;
    const { lastFrame } = renderTui(
      <Markdown fileOpener={undefined}>{nestedList}</Markdown>,
    );

    const outputWithAnsi = lastFrame();
    const i4 = " ".repeat(4);
    const expectedNestedList = `\
Paragraph before bulleted list.

${i4}* item 1
${i4}${i4}* subitem 1
${i4}${i4}* subitem 2
${i4}* item 2`;
    expect(outputWithAnsi).toBe(expectedNestedList);
  });

  // We had to patch in https://github.com/mikaelbr/marked-terminal/pull/367 to
  // make this work.
  it("ensure sequential subitems with styling to do not get extra newlines", () => {
    // This is a real-world example that exhibits many of the Markdown features
    // we care about. Though the original issue fix this was intended to verify
    // was that even though there is a single newline between the two subitems,
    // the stock version of marked-terminal@7.3.0 was adding an extra newline
    // in the output.
    const nestedList = `\
## 🛠 Core CLI Logic

All of the TypeScript/React code lives under \`src/\`. The main entrypoint for argument parsing and orchestration is:

### \`src/cli.tsx\`
- Uses **meow** for flags/subcommands and prints the built-in help/usage:
  【F:src/cli.tsx†L49-L53】【F:src/cli.tsx†L55-L60】
- Handles special subcommands (e.g. \`codex completion …\`), \`--config\`, API-key validation, then either:
  - Spawns the **AgentLoop** for the normal multi-step prompting/edits flow, or
  - Runs **single-pass** mode if \`--full-context\` is set.

`;
    const { lastFrame } = renderTui(
      <Markdown fileOpener={"vscode"} cwd="/home/user/codex">
        {nestedList}
      </Markdown>,
    );

    const outputWithAnsi = lastFrame();

    // Note that the line with two citations gets split across two lines.
    // While the underlying ANSI content is long such that the split appears to
    // be merited, the rendered output is considerably shorter and ideally it
    // would be a single line.
    const expectedNestedList = `${GREEN}${BOLD}## 🛠 Core CLI Logic${BOLD_OFF}${COLOR_OFF}

All of the TypeScript/React code lives under ${YELLOW}src/${COLOR_OFF}. The main entrypoint for argument parsing and
orchestration is:

${GREEN}${BOLD}### ${YELLOW}src/cli.tsx${COLOR_OFF}${BOLD_OFF}

    * Uses ${BOLD}meow${BOLD_OFF} for flags/subcommands and prints the built-in help/usage:
      ${BLUE}src/cli.tsx:49 (${LINK_ON}vscode://file/home/user/codex/src/cli.tsx:49${LINK_OFF})${COLOR_OFF} ${BLUE}src/cli.tsx:55 ${COLOR_OFF}
${BLUE}(${LINK_ON}vscode://file/home/user/codex/src/cli.tsx:55${LINK_OFF})${COLOR_OFF}
    * Handles special subcommands (e.g. ${YELLOW}codex completion …${COLOR_OFF}), ${YELLOW}--config${COLOR_OFF}, API-key validation, then
either:
        * Spawns the ${BOLD}AgentLoop${BOLD_OFF} for the normal multi-step prompting/edits flow, or
        * Runs ${BOLD}single-pass${BOLD_OFF} mode if ${YELLOW}--full-context${COLOR_OFF} is set.`;

    expect(toDiffableString(outputWithAnsi)).toBe(
      toDiffableString(expectedNestedList),
    );
  });

  it("citations should get converted to hyperlinks when stdout supports them", () => {
    const { lastFrame } = renderTui(
      <Markdown fileOpener={"vscode"} cwd="/foo/bar">
        File with TODO: 【F:src/approvals.ts†L40】
      </Markdown>,
    );

    const expected = `File with TODO: ${BLUE}src/approvals.ts:40 (${LINK_ON}vscode://file/foo/bar/src/approvals.ts:40${LINK_OFF})${COLOR_OFF}`;
    const outputWithAnsi = lastFrame();
    expect(outputWithAnsi).toBe(expected);
  });
});

function toDiffableString(str: string) {
  // The test harness is not able to handle ANSI codes, so we need to escape
  // them, but still give it line-based input so that it can diff the output.
  return str
    .split("\n")
    .map((line) => JSON.stringify(line))
    .join("\n");
}
