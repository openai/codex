import type { BackendId } from "../sessions";

export function makeBackendInstanceKey(
  workspaceFolderUri: string,
  backendId: BackendId,
): string {
  return JSON.stringify([workspaceFolderUri, backendId]);
}

export function parseBackendInstanceKey(key: string): {
  workspaceFolderUri: string;
  backendId: BackendId;
} {
  let parsed: unknown;
  try {
    parsed = JSON.parse(key) as unknown;
  } catch (err) {
    throw new Error(
      `Invalid backend instance key (expected JSON tuple): ${String((err as Error).message ?? err)}`,
    );
  }

  if (!Array.isArray(parsed) || parsed.length !== 2) {
    throw new Error(
      "Invalid backend instance key (expected [workspaceFolderUri, backendId])",
    );
  }

  const workspaceFolderUri = parsed[0];
  const backendId = parsed[1];
  if (
    typeof workspaceFolderUri !== "string" ||
    workspaceFolderUri.length === 0
  ) {
    throw new Error(
      "Invalid backend instance key: workspaceFolderUri must be a non-empty string",
    );
  }
  if (
    backendId !== "codex" &&
    backendId !== "codez" &&
    backendId !== "opencode"
  ) {
    throw new Error(
      "Invalid backend instance key: backendId must be codex|codez|opencode",
    );
  }
  return { workspaceFolderUri, backendId };
}
