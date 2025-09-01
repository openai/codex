#!/usr/bin/env node
/**
 * Codex CLI postinstall script
 * Downloads platform-specific binary to reduce package size
 */

import fs from "fs";
import path from "path";
import { pipeline } from "stream/promises";
import { createGunzip } from "zlib";
import { fileURLToPath } from "url";
import { extract } from "tar-stream";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function getPlatformInfo() {
  const { platform, arch } = process;

  let targetTriple = null;
  switch (platform) {
    case "linux":
    case "android":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-unknown-linux-musl";
          break;
        case "arm64":
          targetTriple = "aarch64-unknown-linux-musl";
          break;
      }
      break;
    case "darwin":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-apple-darwin";
          break;
        case "arm64":
          targetTriple = "aarch64-apple-darwin";
          break;
      }
      break;
    case "win32":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-pc-windows-msvc.exe";
          break;
        case "arm64":
        // We do not build this today, fall through...
        default:
          break;
      }
      break;
  }

  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${platform} (${arch})`);
  }

  return { targetTriple, platform, arch };
}

// Fetch with optional proxy support
async function createFetch() {
  try {
    const { HttpsProxyAgent } = await import("https-proxy-agent");
    const proxyUrl = process.env.HTTPS_PROXY || process.env.HTTP_PROXY;

    if (proxyUrl) {
      const agent = new HttpsProxyAgent(proxyUrl);
      return (url, options = {}) => fetch(url, { ...options, agent });
    }
  } catch {
    if (process.env.DEBUG) {
      console.log("Using built-in fetch (no proxy agent)");
    }
  }
  return fetch;
}

// Retry wrapper for fetch
async function fetchWithRetry(fetchFn, url, retries = 3) {
  let lastError;
  for (let i = 0; i < retries; i++) {
    try {
      const res = await fetchFn(url);
      if (res.ok) return res;
      throw new Error(`HTTP ${res.status} ${res.statusText}`);
    } catch (err) {
      lastError = err;
      if (process.env.DEBUG) {
        console.log(`Fetch attempt ${i + 1} failed: ${err.message}`);
      }
      if (i < retries - 1) {
        await new Promise((r) => setTimeout(r, 2 ** i * 1000));
      }
    }
  }
  throw lastError;
}

// Extract binary from tar.gz
async function extractBinaryFromTarGz(
  response,
  outputPath,
  expectedBinaryName,
) {
  const extractStream = extract();
  let found = false;
  let extractedFilename = null;

  return new Promise((resolve, reject) => {
    extractStream.on("entry", (header, stream, next) => {
      const baseName = path.basename(header.name);
      if (header.type === "file" && baseName === expectedBinaryName) {
        found = true;
        extractedFilename = baseName;

        const writeStream = fs.createWriteStream(outputPath);
        stream.pipe(writeStream);

        writeStream.on("finish", next);
        writeStream.on("error", reject);
      } else {
        stream.on("end", next);
        stream.resume();
      }
    });

    extractStream.on("finish", () => {
      if (!found) {
        reject(new Error("Binary not found in tar archive"));
      } else {
        resolve(extractedFilename);
      }
    });

    extractStream.on("error", reject);

    pipeline(response.body, createGunzip(), extractStream).catch(reject);
  });
}

// Download platform-specific binary
async function downloadBinary(targetTriple) {
  const packageJson = JSON.parse(
    await fs.promises.readFile(path.join(__dirname, "package.json"), "utf8"),
  );

  const binaryVersion = packageJson.binaryVersion;

  const downloadBinaryName = `codex-${targetTriple}`;
  const localBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
  const downloadUrl =
    `https://github.com/openai/codex/releases/download/` +
    `${binaryVersion}/${downloadBinaryName}.tar.gz`;

  const binDir = path.join(__dirname, "bin");
  await fs.promises.mkdir(binDir, { recursive: true });

  const outputPath = path.join(binDir, localBinaryName);

  if (process.env.DEBUG) {
    console.log(`Download target: ${downloadBinaryName}`);
    console.log(`URL: ${downloadUrl}`);
  } else {
    console.log(`Downloading ${downloadBinaryName}...`);
  }

  try {
    const fetchFn = await createFetch();
    const response = await fetchWithRetry(fetchFn, downloadUrl, 3);

    if (!response.ok) {
      throw new Error(
        `Download failed: ${response.status} ${response.statusText}`,
      );
    }

    if (process.env.DEBUG) {
      console.log(`Extracting ${downloadBinaryName} to ${outputPath}`);
    }

    const extractedFilename = await extractBinaryFromTarGz(
      response,
      outputPath,
      downloadBinaryName,
    );

    if (process.platform !== "win32") {
      await fs.promises.chmod(outputPath, 0o755);
    }

    console.log(`Installed: ${extractedFilename} -> ${localBinaryName}`);
  } catch (error) {
    try {
      await fs.promises.unlink(outputPath);
    } catch {}
    throw error;
  }
}

// Main execution
async function main() {
  try {
    const { targetTriple } = getPlatformInfo();
    if (process.env.DEBUG) {
      console.log(`Detected platform: ${targetTriple}`);
    }

    await downloadBinary(targetTriple);

    console.log(`Codex CLI installation completed!`);
  } catch (error) {
    console.error("Postinstall failed:", error.message);
    console.error("\nTroubleshooting:");
    console.error("   1. Check your internet connection");
    console.error("   2. Verify the GitHub release exists for your platform");
    console.error("   3. Check proxy settings if in a corporate environment");
    console.error("   4. Run with DEBUG=1 for verbose logs");
    process.exit(1);
  }
}

main();
