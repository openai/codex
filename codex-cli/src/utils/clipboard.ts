import { fileTypeFromBuffer } from "file-type";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

function makeTempFilePath(extension: string = "png"): string {
  const safeExt = extension.replace(/[^a-z0-9]/gi, "").toLowerCase() || "png";
  const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return path.join(os.tmpdir(), `codex-clipboard-${unique}.${safeExt}`);
}

function sanitizeForPowerShell(value: string): string {
  return value.replace(/'/g, "''");
}

function sanitizeForAppleScript(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

async function tryCaptureViaCommand(
  command: string,
  args: Array<string>,
  maxBytes: number = 20 * 1024 * 1024,
): Promise<Buffer | null> {
  return await new Promise((resolve) => {
    let resolved = false;
    try {
      const child = spawn(command, args);
      const chunks: Array<Buffer> = [];
      let total = 0;

      child.stdout.on("data", (chunk: Buffer) => {
        if (resolved) {
          return;
        }
        total += chunk.length;
        if (total > maxBytes) {
          resolved = true;
          child.kill();
          resolve(null);
          return;
        }
        chunks.push(chunk);
      });

      child.once("error", () => {
        if (!resolved) {
          resolved = true;
          resolve(null);
        }
      });

      child.once("close", (code) => {
        if (resolved) {
          return;
        }
        resolved = true;
        if (code === 0 && chunks.length > 0) {
          resolve(Buffer.concat(chunks));
        } else {
          resolve(null);
        }
      });
    } catch (err) {
      if (!resolved) {
        resolved = true;
        resolve(null);
      }
    }
  });
}

async function captureWindowsClipboardImage(): Promise<string | null> {
  const filePath = makeTempFilePath("png");
  const psPath = sanitizeForPowerShell(filePath);
  const script = [
    "Add-Type -AssemblyName System.Drawing;",
    "$img = Get-Clipboard -Format Image;",
    "if ($img -eq $null) { exit 1 }",
    `$path = '${psPath}';`,
    "$dir = Split-Path -Parent $path;",
    "if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }",
    "$img.Save($path, [System.Drawing.Imaging.ImageFormat]::Png);",
    "Write-Output $path;",
  ].join(" ");

  const buffer = await tryCaptureViaCommand("powershell", ["-NoProfile", "-Command", script]);
  if (!buffer) {
    return null;
  }
  try {
    await fs.access(filePath);
    return filePath;
  } catch {
    return null;
  }
}

async function captureMacClipboardImage(): Promise<string | null> {
  const filePath = makeTempFilePath("png");
  const escapedPath = sanitizeForAppleScript(filePath);
  const script = [
    `set theFile to POSIX file \"${escapedPath}\"`,
    "try",
    "  set pngData to the clipboard as «class PNGf»",
    "on error",
    "  return \"\"",
    "end try",
    "set outFile to open for access theFile with write permission",
    "try",
    "  set eof outFile to 0",
    "  write pngData to outFile",
    "on error",
    "  close access outFile",
    "  return \"\"",
    "end try",
    "close access outFile",
    "return POSIX path of theFile",
  ].join(" \n");

  const buffer = await tryCaptureViaCommand("osascript", ["-e", script]);
  if (!buffer) {
    return null;
  }
  const output = buffer.toString("utf8").trim();
  if (!output) {
    return null;
  }
  try {
    await fs.access(filePath);
    return filePath;
  } catch {
    return null;
  }
}

async function captureLinuxClipboardImage(): Promise<string | null> {
  const attempts: Array<{ command: string; args: Array<string> }> = [
    { command: "wl-paste", args: ["--no-newline", "--type", "image/png"] },
    { command: "wl-paste", args: ["--no-newline", "--type", "image/jpeg"] },
    { command: "xclip", args: ["-selection", "clipboard", "-t", "image/png", "-o"] },
    { command: "xclip", args: ["-selection", "clipboard", "-t", "image/jpeg", "-o"] },
  ];

  for (const { command, args } of attempts) {
    const buffer = await tryCaptureViaCommand(command, args);
    if (!buffer) {
      continue;
    }
    const kind = await fileTypeFromBuffer(buffer);
    const ext = kind?.ext ?? "png";
    const filePath = makeTempFilePath(ext);
    try {
      await fs.writeFile(filePath, buffer);
      return filePath;
    } catch {
      // Try next attempt if writing fails
    }
  }
  return null;
}

export async function captureClipboardImage(): Promise<string | null> {
  try {
    if (process.platform === "win32") {
      return await captureWindowsClipboardImage();
    }
    if (process.platform === "darwin") {
      return await captureMacClipboardImage();
    }
    if (process.platform === "linux") {
      return await captureLinuxClipboardImage();
    }
    return null;
  } catch (err) {
    return null;
  }
}
