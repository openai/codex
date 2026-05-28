#!/usr/bin/env node
import {
  access,
  mkdir,
  readFile,
  rename,
  stat,
  writeFile,
} from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import process from "node:process";
import { inspect } from "node:util";

const DEFAULT_MANUAL_URL = "https://developers.openai.com/codex/codex-manual.md";
const CACHE_FILE_NAME = "codex-manual.md";
const OUTLINE_FILE_NAME = "codex-manual.outline.md";
const HASH_HEADER = "x-content-sha256";

class ManualFetchError extends Error {
  constructor(message, options) {
    super(message, options);
    this.name = "ManualFetchError";
  }
}

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

const withTimeout = async (promiseFactory, timeoutMs) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await promiseFactory(controller.signal);
  } finally {
    clearTimeout(timeout);
  }
};

const requestManual = async (url, { method, timeoutMs }) => {
  let response;
  try {
    response = await withTimeout(
      (signal) =>
        fetch(url, {
          method,
          headers: { "User-Agent": "codex-openai-docs" },
          signal,
        }),
      timeoutMs
    );
  } catch (error) {
    throw new ManualFetchError(`${method} ${url} could not be fetched.`, {
      cause: error,
    });
  }

  if (!response.ok) {
    throw new ManualFetchError(
      `${method} ${url} failed with HTTP ${response.status}.`
    );
  }

  return response;
};

const readHeaderSha = (response) => {
  const value = response.headers.get(HASH_HEADER);
  if (!value || !/^[a-f0-9]{64}$/i.test(value)) {
    throw new ManualFetchError(`Manual response is missing ${HASH_HEADER}.`);
  }
  return value.toLowerCase();
};

const nearestExistingParent = async (target) => {
  let current = target;
  while (true) {
    try {
      const info = await stat(current);
      return info.isDirectory() ? current : null;
    } catch (error) {
      if (error?.code !== "ENOENT") return null;
    }

    const parent = path.dirname(current);
    if (parent === current) return null;
    current = parent;
  }
};

const usableCacheDir = async (cacheDir) => {
  if (!cacheDir) return null;
  const resolved = path.resolve(cacheDir);

  try {
    const info = await stat(resolved);
    if (!info.isDirectory()) return null;
  } catch (error) {
    if (error?.code !== "ENOENT") return null;
  }

  const parent = await nearestExistingParent(resolved);
  if (!parent) return null;

  try {
    await access(parent, fsConstants.W_OK | fsConstants.X_OK);
  } catch {
    return null;
  }

  return resolved;
};

const cacheFilePath = (cacheDir) => path.join(cacheDir, CACHE_FILE_NAME);

const outlineFilePath = (cacheDir) => path.join(cacheDir, OUTLINE_FILE_NAME);

const manualLines = (manual) => {
  const lines = manual.replace(/\r\n/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
};

const sectionTitle = (rawTitle) =>
  rawTitle.replace(/\s+#+\s*$/, "").replace(/\s+/g, " ").trim();

const buildOutline = (manual) => {
  const lines = manualLines(manual);
  const headings = [];
  let inFence = false;

  lines.forEach((line, index) => {
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence;
      return;
    }
    if (inFence) return;

    const match = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (!match) return;

    const level = match[1].length;
    if (level < 2 || level > 3) return;

    headings.push({
      level,
      title: sectionTitle(match[2]),
      startLine: index + 1,
      endLine: lines.length,
    });
  });

  for (let index = 0; index < headings.length; index += 1) {
    const heading = headings[index];
    const nextPeer = headings
      .slice(index + 1)
      .find((candidate) => candidate.level <= heading.level);
    if (nextPeer) {
      heading.endLine = nextPeer.startLine - 1;
    }
  }

  if (headings.length === 0) {
    return {
      headingCount: 0,
      lineCount: lines.length,
      text: "No markdown headings found.",
    };
  }

  const minLevel = Math.min(...headings.map((heading) => heading.level));
  return {
    headingCount: headings.length,
    lineCount: lines.length,
    text: headings
      .map((heading) => {
        const indent = "  ".repeat(heading.level - minLevel);
        return `${indent}- ${heading.title} (lines ${heading.startLine}-${heading.endLine})`;
      })
      .join("\n"),
  };
};

const outlineMarkdown = (outline) => `# Codex Manual Outline\n\n${outline.text}\n`;

const manualStatusLine = (status) =>
  status.cacheStatus === "hit"
    ? "Manual status: local manual was already current."
    : "Manual status: local manual was updated.";

const formatResult = ({ status, outlineText }) =>
  [
    `Manual path: ${status.manualPath}`,
    `Outline path: ${status.outlinePath}`,
    manualStatusLine(status),
    "",
    outlineText,
  ].join("\n");

const readCachedManual = async (cacheDir, expectedSha256) => {
  try {
    const manual = await readFile(cacheFilePath(cacheDir), "utf8");
    return sha256(manual) === expectedSha256 ? manual : null;
  } catch {
    return null;
  }
};

const writeCachedManual = async (cacheDir, manual) => {
  await mkdir(cacheDir, { recursive: true });
  const tmpPath = path.join(cacheDir, `.${CACHE_FILE_NAME}.tmp`);
  await writeFile(tmpPath, manual, "utf8");
  await rename(tmpPath, cacheFilePath(cacheDir));
};

const readCachedOutline = async (cacheDir) => {
  try {
    return await readFile(outlineFilePath(cacheDir), "utf8");
  } catch {
    return null;
  }
};

const writeOutline = async (cacheDir, outlineText) => {
  await mkdir(cacheDir, { recursive: true });
  const tmpPath = path.join(cacheDir, `.${OUTLINE_FILE_NAME}.tmp`);
  await writeFile(tmpPath, outlineText, "utf8");
  await rename(tmpPath, outlineFilePath(cacheDir));
};

const fetchCodexManual = async ({
  manualUrl = DEFAULT_MANUAL_URL,
  cacheDir,
  timeoutMs = 30000,
} = {}) => {
  if (!cacheDir) {
    throw new ManualFetchError(
      "--cache-dir is required so the manual can be searched with local file tools."
    );
  }

  const resolvedCacheDir = await usableCacheDir(cacheDir);
  if (!resolvedCacheDir) {
    throw new ManualFetchError(
      "Manual cache directory is unavailable; use OpenAI Docs MCP fallback."
    );
  }

  const headResponse = await requestManual(manualUrl, {
    method: "HEAD",
    timeoutMs,
  });
  const expectedSha256 = readHeaderSha(headResponse);
  const manualPath = cacheFilePath(resolvedCacheDir);
  const outlinePath = outlineFilePath(resolvedCacheDir);
  const checkedAt = new Date().toISOString();

  const cachedManual = await readCachedManual(resolvedCacheDir, expectedSha256);
  if (cachedManual !== null) {
    const outlineText = await readCachedOutline(resolvedCacheDir);
    if (outlineText !== null) {
      return {
        outlineText,
        status: {
          manualUrl,
          headerSha256: expectedSha256,
          fetchedManualSha256: expectedSha256,
          manualHashMatches: true,
          cacheStatus: "hit",
          cacheDir: resolvedCacheDir,
          manualPath,
          outlinePath,
          checkedAt,
        },
      };
    }
  }

  const getResponse = await requestManual(manualUrl, {
    method: "GET",
    timeoutMs,
  });
  const getHeaderSha256 = readHeaderSha(getResponse);
  if (getHeaderSha256 !== expectedSha256) {
    throw new ManualFetchError(
      `${HASH_HEADER} changed between HEAD and GET for ${manualUrl}.`
    );
  }

  const manualText = await getResponse.text();
  const actualSha256 = sha256(manualText);
  const manualHashMatches = actualSha256 === expectedSha256;
  if (!manualHashMatches) {
    throw new ManualFetchError(
      `${HASH_HEADER} did not match the fetched manual body for ${manualUrl}.`
    );
  }

  await writeCachedManual(resolvedCacheDir, manualText);
  const outline = buildOutline(manualText);
  const outlineText = outlineMarkdown(outline);
  await writeOutline(resolvedCacheDir, outlineText);

  return {
    outlineText,
    status: {
      manualUrl,
      headerSha256: expectedSha256,
      fetchedManualSha256: actualSha256,
      manualHashMatches,
      cacheStatus: "updated",
      cacheDir: resolvedCacheDir,
      manualPath,
      outlinePath,
      checkedAt,
      lineCount: outline.lineCount,
      headingCount: outline.headingCount,
    },
  };
};

const parseArgs = (argv) => {
  const args = {
    manualUrl: DEFAULT_MANUAL_URL,
    cacheDir: undefined,
    timeoutMs: 30000,
    statusJson: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--manual-url") {
      args.manualUrl = argv[++index];
    } else if (arg === "--cache-dir") {
      args.cacheDir = argv[++index];
    } else if (arg === "--timeout-ms") {
      args.timeoutMs = Number(argv[++index]);
    } else if (arg === "--status-json") {
      args.statusJson = true;
    } else {
      throw new ManualFetchError(`Unknown argument: ${arg}`);
    }
  }

  if (!args.manualUrl) {
    throw new ManualFetchError("--manual-url cannot be empty.");
  }
  if (!Number.isFinite(args.timeoutMs) || args.timeoutMs <= 0) {
    throw new ManualFetchError("--timeout-ms must be a positive number.");
  }

  return args;
};

const main = async () => {
  const args = parseArgs(process.argv.slice(2));
  const { outlineText, status } = await fetchCodexManual(args);

  process.stdout.write(formatResult({ status, outlineText }));

  if (args.statusJson) {
    console.error(JSON.stringify(status));
  }
};

const envProxyHint = () => {
  const proxyConfigured =
    process.env.HTTP_PROXY ||
    process.env.HTTPS_PROXY ||
    process.env.http_proxy ||
    process.env.https_proxy;
  if (!proxyConfigured || process.env.NODE_USE_ENV_PROXY === "1") {
    return null;
  }

  return "Hint: HTTP(S)_PROXY is set but NODE_USE_ENV_PROXY is not. Retry with `NODE_USE_ENV_PROXY=1 node scripts/fetch-codex-manual.mjs ...` so Node fetch can use the session proxy.";
};

const formatErrorDetails = (error) =>
  inspect(error, {
    breakLength: 120,
    colors: false,
    compact: false,
    depth: 8,
  });

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  const hint = envProxyHint();
  if (hint) {
    console.error(hint);
  }
  console.error("");
  console.error("Details:");
  console.error(formatErrorDetails(error));
  process.exitCode = 1;
});

export { DEFAULT_MANUAL_URL, fetchCodexManual };
