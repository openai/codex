export function resolveTargetTriple(
  platform: NodeJS.Platform,
  arch: NodeJS.Architecture,
): string {
  if (platform === "linux") {
    if (arch === "x64") {
      return "x86_64-unknown-linux-musl";
    }
    if (arch === "arm64") {
      return "aarch64-unknown-linux-musl";
    }
  } else if (platform === "darwin") {
    if (arch === "x64") {
      return "x86_64-apple-darwin";
    }
    if (arch === "arm64") {
      return "aarch64-apple-darwin";
    }
  }
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}
