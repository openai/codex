import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";

export type AgentSummary = {
  name: string;
  description: string;
  color: string | null;
  path: string;
  source: "repo" | "home";
};

export async function listAgentsFromDisk(
  cwdFsPath: string,
): Promise<{ agents: AgentSummary[]; errors: string[]; gitRoot: string | null }> {
  const errors: string[] = [];
  const agents: AgentSummary[] = [];
  const seen = new Set<string>();

  const gitRoot = await findGitRoot(cwdFsPath, errors);
  const roots: Array<{ dir: string; source: "repo" | "home" }> = [];
  if (gitRoot)
    roots.push({ dir: path.join(gitRoot, ".codex", "agents"), source: "repo" });
  roots.push({ dir: path.join(resolveCodexHome(), "agents"), source: "home" });

  for (const root of roots) {
    const names = await listMarkdownStems(root.dir, errors);
    for (const name of names) {
      if (!seen.add(name)) continue;
      if (!isValidAgentName(name)) {
        errors.push(`${root.source}: invalid agent name: ${name}`);
        continue;
      }
      const filePath = path.join(root.dir, `${name}.md`);
      let content: string;
      try {
        content = await fs.readFile(filePath, "utf8");
      } catch (err) {
        errors.push(
          `${root.source}: failed to read ${filePath}: ${String((err as Error).message ?? err)}`,
        );
        continue;
      }
      const parsed = parseAgentFrontmatter(content);
      if (!parsed.ok) {
        errors.push(`${root.source}: failed to parse ${filePath}: ${parsed.error}`);
        continue;
      }
      agents.push({
        name,
        description: parsed.description,
        color: parsed.color,
        path: filePath,
        source: root.source,
      });
    }
  }

  agents.sort((a, b) => a.name.localeCompare(b.name));
  return { agents, errors, gitRoot };
}

function resolveCodexHome(): string {
  const env = process.env["CODEX_HOME"];
  if (env && env.trim()) return env.trim();
  return path.join(os.homedir(), ".codex");
}

async function findGitRoot(start: string, errors: string[]): Promise<string | null> {
  let cur = path.resolve(start);
  for (let i = 0; i < 50; i += 1) {
    const gitPath = path.join(cur, ".git");
    try {
      const st = await fs.stat(gitPath);
      if (st.isDirectory() || st.isFile()) return cur;
    } catch (err) {
      const code = (err as NodeJS.ErrnoException).code;
      if (code && code !== "ENOENT" && code !== "ENOTDIR") {
        errors.push(
          `failed to stat ${gitPath}: ${String((err as Error).message ?? err)}`,
        );
      }
    }

    const parent = path.dirname(cur);
    if (parent === cur) break;
    cur = parent;
  }
  return null;
}

async function listMarkdownStems(dir: string, errors: string[]): Promise<string[]> {
  try {
    const entries = await fs.readdir(dir, { withFileTypes: true });
    const out: string[] = [];
    for (const e of entries) {
      if (!e.isFile()) continue;
      if (path.extname(e.name).toLowerCase() !== ".md") continue;
      const stem = path.parse(e.name).name.trim();
      if (!stem) continue;
      out.push(stem);
    }
    out.sort();
    return out;
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code !== "ENOENT" && code !== "ENOTDIR") {
      errors.push(`failed to read dir ${dir}: ${String((err as Error).message ?? err)}`);
    }
    return [];
  }
}

function isValidAgentName(name: string): boolean {
  if (!name) return false;
  if (name === "." || name === "..") return false;
  if (name.includes("/") || name.includes("\\")) return false;
  return /^[A-Za-z0-9_-]+$/.test(name);
}

function parseAgentFrontmatter(
  content: string,
): { ok: true; description: string; color: string | null } | { ok: false; error: string } {
  const lines = content.split(/\r?\n/);
  if ((lines[0] ?? "").trim() !== "---") {
    return { ok: false, error: "missing YAML frontmatter (expected starting ---)" };
  }
  let desc: string | null = null;
  let color: string | null = null;
  let foundClose = false;

  for (let i = 1; i < lines.length; i += 1) {
    const raw = lines[i] ?? "";
    const trimmed = raw.trim();
    if (trimmed === "---") {
      foundClose = true;
      break;
    }
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf(":");
    if (idx <= 0) continue;
    const key = trimmed.slice(0, idx).trim().toLowerCase();
    let val = trimmed.slice(idx + 1).trim();
    if (val.length >= 2) {
      const first = val[0];
      const last = val[val.length - 1];
      if ((first === "\"" && last === "\"") || (first === "'" && last === "'")) {
        val = val.slice(1, -1);
      }
    }
    if (key === "description") desc = val;
    if (key === "color") color = val || null;
  }

  if (!foundClose) {
    return { ok: false, error: "unterminated YAML frontmatter (missing closing ---)" };
  }

  if (!desc || !desc.trim()) {
    return { ok: false, error: "missing required frontmatter field: description" };
  }

  return { ok: true, description: desc.trim(), color: color ? color.trim() : null };
}
