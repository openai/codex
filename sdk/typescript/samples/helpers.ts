import path from "node:path";

export function codexPathOverride() {
  return (
    process.env.CODEX_EXECUTABLE ??
    path.join(process.cwd(), "vendor", "x86_64-unknown-linux-musl", "codex", "codex")
  );
}
