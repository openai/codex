import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptFileName = fileURLToPath(import.meta.url);
const scriptDirName = path.dirname(scriptFileName);

export function findCodexPath(codexPathOverride: string | null = null): string {
  if (codexPathOverride) {
    return codexPathOverride;
  }

  const { platform, arch } = process;

  let targetTriple: string | null = null;
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
        default:
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
        default:
          break;
      }
      break;
    case "win32":
      switch (arch) {
        case "x64":
          targetTriple = "x86_64-pc-windows-msvc";
          break;
        case "arm64":
          targetTriple = "aarch64-pc-windows-msvc";
          break;
        default:
          break;
      }
      break;
    default:
      break;
  }

  if (!targetTriple) {
    throw new Error(`Unsupported platform: ${platform} (${arch})`);
  }

  const vendorRoot = path.join(scriptDirName, "..", "vendor");
  const archRoot = path.join(vendorRoot, targetTriple);
  const codexBinaryName = process.platform === "win32" ? "codex.exe" : "codex";
  return path.join(archRoot, "codex", codexBinaryName);
}
