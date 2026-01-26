import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const codexCliRoot = path.resolve(__dirname, "..");
const cliEntrypoint = path.join(codexCliRoot, "bin", "codex.js");

function resolveTargetTriple() {
  const { platform, arch } = process;
  switch (platform) {
    case "linux":
    case "android":
      switch (arch) {
        case "x64":
          return "x86_64-unknown-linux-musl";
        case "arm64":
          return "aarch64-unknown-linux-musl";
        default:
          break;
      }
      break;
    case "darwin":
      switch (arch) {
        case "x64":
          return "x86_64-apple-darwin";
        case "arm64":
          return "aarch64-apple-darwin";
        default:
          break;
      }
      break;
    case "win32":
      switch (arch) {
        case "x64":
          return "x86_64-pc-windows-msvc";
        case "arm64":
          return "aarch64-pc-windows-msvc";
        default:
          break;
      }
      break;
    default:
      break;
  }
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

async function withStubBinary(run) {
  const vendorRoot = await fs.mkdtemp(
    path.join(os.tmpdir(), "codex-cli-vendor-"),
  );
  const targetTriple = resolveTargetTriple();
  const binDir = path.join(vendorRoot, targetTriple, "codex");
  const binName = process.platform === "win32" ? "codex.exe" : "codex";
  const stubPath = path.join(binDir, binName);
  await fs.mkdir(binDir, { recursive: true });
  await fs.writeFile(
    stubPath,
    `#!${process.execPath}\nsetInterval(() => {}, 1000);\n`,
  );
  await fs.chmod(stubPath, 0o755);

  try {
    await run({ vendorRoot });
  } finally {
    await fs.rm(vendorRoot, { recursive: true, force: true });
  }
}

test("re-emits child signal so parent exits with signal semantics", async (t) => {
  if (process.platform === "win32") {
    t.skip("Signal exit semantics differ on Windows.");
    return;
  }

  await withStubBinary(async ({ vendorRoot }) => {
    const child = spawn(process.execPath, [cliEntrypoint], {
      env: {
        ...process.env,
        CODEX_CLI_VENDOR_ROOT: vendorRoot,
      },
      stdio: "ignore",
    });

    const result = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error("codex-cli did not exit after SIGTERM"));
      }, 3000);
      child.on("exit", (code, signal) => {
        clearTimeout(timer);
        resolve({ code, signal });
      });
      child.on("error", (err) => {
        clearTimeout(timer);
        reject(err);
      });
      setTimeout(() => {
        child.kill("SIGTERM");
      }, 200);
    });

    assert.equal(result.signal, "SIGTERM");
    assert.equal(result.code, null);
  });
});
