import { promises as fs } from "node:fs";
import path from "node:path";

const DEFAULT_GUARDRAILS_DIR = ".guardrails";

/**
 * Load guardrail markdown files from the current project root.
 *
 * Files are returned in sorted order with a markdown heading for each file.
 *
 * @param {{ cwd?: string }} [options]
 * @returns {Promise<string>} Combined guardrail contents.
 */
export async function loadGuardrails(options = {}) {
  const { cwd = process.cwd() } = options;
  const guardrailsDir = path.join(cwd, DEFAULT_GUARDRAILS_DIR);

  let entries;
  try {
    entries = await fs.readdir(guardrailsDir, { withFileTypes: true });
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return "";
    }
    throw error;
  }

  const markdownFiles = entries
    .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".md"))
    .map((entry) => entry.name)
    .sort((a, b) => a.localeCompare(b));

  if (markdownFiles.length === 0) {
    return "";
  }

  const sections = [];
  for (const fileName of markdownFiles) {
    const filePath = path.join(guardrailsDir, fileName);
    const fileContent = await fs.readFile(filePath, "utf8");
    const heading = path.parse(fileName).name;
    const normalizedContent = fileContent.replace(/\s+$/u, "");
    const sectionBody = normalizedContent ? `\n\n${normalizedContent}` : "";
    sections.push(`## Guardrail: ${heading}${sectionBody}`);
  }

  return sections.join("\n\n");
}
