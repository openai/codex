import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";

import { describe, it } from "@jest/globals";

import { codexExecPath } from "./testCodex";

const conformanceTimeoutMs = 300_000;
const reviewerRegressionTimeoutMs = 180_000;

function requireFile(filePath: string, description: string): string {
  if (!existsSync(filePath)) {
    throw new Error(`MCP conformance ${description} does not exist: ${filePath}`);
  }

  return filePath;
}

function conformanceDirectory(): string {
  const repositoryRoot = path.resolve(process.cwd(), "../..");
  const candidates = [
    path.join(repositoryRoot, "public", "scripts", "mcp_conformance"),
    path.join(repositoryRoot, "scripts", "mcp_conformance"),
  ];

  for (const candidate of candidates) {
    if (existsSync(path.join(candidate, "run_codex_compliance.py"))) {
      return candidate;
    }
  }

  throw new Error(`MCP conformance harness is missing; checked ${candidates.join(" and ")}`);
}

describe("MCP client conformance", () => {
  it(
    "does not introduce shipping, intermediate, or modern official conformance regressions",
    () => {
      const directory = conformanceDirectory();
      const require = createRequire(import.meta.url);
      const conformancePackage = require.resolve("@modelcontextprotocol/conformance/package.json");
      const officialConformanceCli = requireFile(
        path.join(path.dirname(conformancePackage), "dist", "index.js"),
        "pinned official CLI",
      );
      const codexBinary = requireFile(codexExecPath, "built Codex binary");
      const baseline = requireFile(
        path.join(directory, "regression-baseline-v1.json"),
        "committed regression baseline",
      );
      const reportDirectory = process.env.RUNNER_TEMP ?? os.tmpdir();
      const reportPath = path.join(reportDirectory, `codex-mcp-conformance-${process.pid}.json`);
      const result = spawnSync(
        "python3",
        [
          path.join(directory, "run_codex_compliance.py"),
          codexBinary,
          "--mode",
          "all",
          "--transport",
          "all",
          "--auth",
          "--conformance-cli",
          officialConformanceCli,
          "--baseline-report",
          baseline,
          "--report",
          reportPath,
        ],
        {
          encoding: "utf8",
          maxBuffer: 16 * 1024 * 1024,
          timeout: conformanceTimeoutMs - 10_000,
        },
      );

      if (result.error) {
        throw new Error(`MCP conformance runner could not complete: ${result.error.message}`, {
          cause: result.error,
        });
      }

      if (result.status !== 0) {
        throw new Error(
          [
            `MCP conformance regression gate exited with ${result.status ?? result.signal}.`,
            `Report: ${reportPath}`,
            result.stdout,
            result.stderr,
          ]
            .filter(Boolean)
            .join("\n"),
        );
      }
    },
    conformanceTimeoutMs,
  );

  it(
    "does not introduce production reviewer or catalog-boundary regressions",
    () => {
      const directory = conformanceDirectory();
      const codexBinary = requireFile(codexExecPath, "built Codex binary");
      const reviewer = requireFile(
        path.join(directory, "review_regressions.py"),
        "production reviewer regression runner",
      );
      const baseline = requireFile(
        path.join(directory, "review-regression-baseline-v1.json"),
        "committed production reviewer regression baseline",
      );
      const reportDirectory = process.env.RUNNER_TEMP ?? os.tmpdir();
      const reportPath = path.join(reportDirectory, `codex-mcp-review-${process.pid}.json`);
      const result = spawnSync(
        "python3",
        [
          reviewer,
          codexBinary,
          "--mode",
          "all",
          "--baseline-report",
          baseline,
          "--report",
          reportPath,
        ],
        {
          encoding: "utf8",
          maxBuffer: 16 * 1024 * 1024,
          timeout: reviewerRegressionTimeoutMs - 10_000,
        },
      );

      if (result.error) {
        throw new Error(
          `MCP production reviewer regression runner could not complete: ${result.error.message}`,
          { cause: result.error },
        );
      }

      if (result.status !== 0) {
        throw new Error(
          [
            `MCP production reviewer regression gate exited with ${result.status ?? result.signal}.`,
            `Report: ${reportPath}`,
            result.stdout,
            result.stderr,
          ]
            .filter(Boolean)
            .join("\n"),
        );
      }
    },
    reviewerRegressionTimeoutMs,
  );
});
