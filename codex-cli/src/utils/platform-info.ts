import os from "os";
import path from "path";

export function getEnvironmentInfo(): { platform: string; shell: string } {
  const platform = `${os.platform()} ${os.arch()} ${os.release()}`;
  const shellPath = process.env["SHELL"] || process.env["ComSpec"] || "";
  const shell = shellPath ? path.basename(shellPath) : "unknown";
  return { platform, shell };
}
