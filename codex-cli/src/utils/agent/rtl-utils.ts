export function forceRTL(text: string): string {
  // Detect if text contains a significant portion of RTL characters (Hebrew, Arabic).
  const rtlChar = /[\u0590-\u05FF\u0600-\u06FF]/u;
  if (text.length === 0) {
    return text;
  }
  const rtlCount = [...text].filter((c) => rtlChar.test(c)).length;
  // If less than 30 % is RTL, leave untouched so mixed LTR logs are fine.
  if (rtlCount / text.length < 0.3) {
    return text;
  }
  const RLE = "\u202B"; // Right‑to‑Left Embedding
  const PDF = "\u202C"; // Pop Directional Formatting
  return `${RLE}${text}${PDF}`;
} 