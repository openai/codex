export function formatTimestamp(seconds?: number): string {
  if (!seconds) {
    return "";
  }
  const date = new Date(seconds * 1000);
  return date.toLocaleString();
}

export function normalizePreview(text: string): string {
  const normalized = text.replace(/\s+/g, " ").trim();
  return normalized;
}

export function nowClock(): string {
  const date = new Date();
  const hh = String(date.getHours()).padStart(2, "0");
  const mm = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}
