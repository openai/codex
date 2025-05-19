#!/usr/bin/env node
import "dotenv/config";

// Exit early if on an older version of Node.js (< 22)
const major = process.versions.node.split(".").map(Number)[0]!;
if (major < 22) {
  // eslint-disable-next-line no-console
  console.error(
    "\n" +
      "Codex CLI requires Node.js version 22 or newer.\n" +
      `You are running Node.js v${process.versions.node}.\n` +
      "Please upgrade Node.js: https://nodejs.org/en/download/\n",
  );
  process.exit(1);
}

// Hack to suppress deprecation warnings (punycode)
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(process as any).noDeprecation = true;

import type { AppRollout } from "./app";
import type { ApprovalPolicy } from "./approvals";
import type { CommandConfirmation } from "./utils/agent/agent-loop";
import type { AppConfig } from "./utils/config";
import type { ResponseItem } from "openai/resources/responses/responses";
import type { ReasoningEffort } from "openai/resources.mjs";

import App from "./app";
import { runSinglePass } from "./cli-singlepass";
import SessionsOverlay from "./components/sessions-overlay.js";
import { AgentLoop } from "./utils/agent/agent-loop";
import { ReviewDecision } from "./utils/agent/review";
import { AutoApprovalMode } from "./utils/auto-approval-mode";
import { checkForUpdates } from "./utils/check-updates";
import {
  loadConfig,
  PRETTY_PRINT,
  INSTRUCTIONS_FILEPATH,
} from "./utils/config";
import {
  getApiKey as fetchApiKey,
  maybeRedeemCredits,
} from "./utils/get-api-key";
import { createInputItem } from "./utils/input-utils";
import { initLogger } from "./utils/logger/log";
import { isModelSupportedForResponses } from "./utils/model-utils.js";
import { parseToolCall } from "./utils/parsers";
import { onExit, setInkRenderer } from "./utils/terminal";
import chalk from "chalk";
import { spawnSync } from "child_process";
import fs from "fs";
import { render } from "ink";
import meow from "meow";
import os from "os";
import path from "path";
import React from "react";

// ---------------------------------------------------------------------------
// EARLY CONFIG LOAD  ▸ ensures approvalMode from ~/.codex/config.json
//                     is available before flags are parsed
// ---------------------------------------------------------------------------
const userConfig: AppConfig = loadConfig(undefined, undefined, {
  cwd: process.cwd(),
});

// Call this early so `tail -F "$TMPDIR/oai-codex/codex-cli-latest.log"` works
// immediately. This must be run with DEBUG=1 for logging to work.
initLogger();

// TODO: migrate to new versions of quiet mode
//
//     -q, --quiet    Non-interactive quiet mode that only prints final message
//     -j, --json     Non-interactive JSON output mode that prints JSON messages

const cli = meow(
  /* … unchanged meow help text … */
  `
  Usage
    $ codex [options] <prompt>
    $ codex completion <bash|zsh|fish>

  Options
    --version                       Print version and exit
    … (trimmed for brevity) …
`,
  {
    importMeta: import.meta,
    autoHelp: true,
    flags: {
      // misc
      help: { type: "boolean", aliases: ["h"] },
      version: { type: "boolean", description: "Print version and exit" },
      /* … all other flag definitions stay exactly the same … */
    },
  },
);

// ---------------------------------------------------------------------------
// Global flag handling
// ---------------------------------------------------------------------------

/* … all code before quiet-mode section is unchanged … */

// ---------------------------------------------------------------------------
// For --quiet, run the cli without user interactions and exit.
// ---------------------------------------------------------------------------
if (cli.flags.quiet) {
  process.env["CODEX_QUIET_MODE"] = "1";
  if (!prompt || prompt.trim() === "") {
    // eslint-disable-next-line no-console
    console.error(
      'Quiet mode requires a prompt string, e.g.,: codex -q "Fix bug #123 in the foobar project"',
    );
    process.exit(1);
  }

  // Determine approval policy for quiet mode based on flags
  const quietApprovalPolicy: ApprovalPolicy =
    cli.flags.fullAuto || cli.flags.approvalMode === "full-auto"
      ? AutoApprovalMode.FULL_AUTO
      : cli.flags.autoEdit || cli.flags.approvalMode === "auto-edit"
        ? AutoApprovalMode.AUTO_EDIT
        // NEW: fall back to userConfig.approvalMode if set
        : config.approvalMode ??
          userConfig.approvalMode ??
          AutoApprovalMode.SUGGEST;

  await runQuietMode({
    prompt,
    imagePaths: imagePaths || [],
    approvalPolicy: quietApprovalPolicy,
    additionalWritableRoots,
    config,
  });
  onExit();
  process.exit(0);
}

// ---------------------------------------------------------------------------
// Interactive default approval policy (unchanged logic, only comment tweak)
// ---------------------------------------------------------------------------
const approvalPolicy: ApprovalPolicy =
  cli.flags.fullAuto || cli.flags.approvalMode === "full-auto"
    ? AutoApprovalMode.FULL_AUTO
    : cli.flags.autoEdit || cli.flags.approvalMode === "auto-edit"
      ? AutoApprovalMode.AUTO_EDIT
      : config.approvalMode ?? AutoApprovalMode.SUGGEST;

/* … rest of file is unchanged … */
