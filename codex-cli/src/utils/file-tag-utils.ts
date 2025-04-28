import fs from "fs";
import path from "path";

/**
 * Replaces @path tokens in the input string with <path>file contents</path> XML blocks for LLM context.
 * Only replaces if the path points to a file; directories are ignored.
 */
export async function expandFileTags(raw: string): Promise<string> {
  const re = /@([\w./~-]+)/g;
  let out = raw;
  type MatchInfo = { index: number; length: number; path: string };
  const matches: Array<MatchInfo> = [];

  for (const m of raw.matchAll(re) as IterableIterator<RegExpMatchArray>) {
    const idx = m.index;
    const captured = m[1];
    if (idx !== undefined && captured) {
      matches.push({ index: idx, length: m[0].length, path: captured });
    }
  }

  // Process in reverse to avoid index shifting.
  for (let i = matches.length - 1; i >= 0; i--) {
    const { index, length, path: p } = matches[i]!;
    const resolved = path.resolve(process.cwd(), p);
    try {
      const st = fs.statSync(resolved);
      if (st.isFile()) {
        const content = fs.readFileSync(resolved, "utf-8");
        const rel = path.relative(process.cwd(), resolved);
        const xml = `<${rel}>\n${content}\n</${rel}>`;
        out = out.slice(0, index) + xml + out.slice(index + length);
      }
    } catch {
      // If path invalid, leave token as is
    }
  }
  return out;
}

/**
 * Collapses <path>content</path> XML blocks back to @path format.
 * This is the reverse operation of expandFileTags.
 */
export function collapseXmlBlocks(text: string): string {
  return text.replace(/<([^\n>]+)>[\s\S]*?<\/\1>/g, (_m, p1: string) => {
    const relPath = p1.trim();
    const displayPath = path.normalize(relPath);
    return "@" + displayPath;
  });
}
