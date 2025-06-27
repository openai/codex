// Based on reference implementation from
// https://cookbook.openai.com/examples/gpt4-1_prompting_guide#reference-implementation-apply_patchpy

import fs from "fs";
import path from "path";
import {
  ADD_FILE_PREFIX,
  DELETE_FILE_PREFIX,
  END_OF_FILE_PREFIX,
  MOVE_FILE_TO_PREFIX,
  PATCH_SUFFIX,
  UPDATE_FILE_PREFIX,
  HUNK_ADD_LINE_PREFIX,
  PATCH_PREFIX,
} from "src/parse-apply-patch";

// -----------------------------------------------------------------------------
// Types & Models
// -----------------------------------------------------------------------------

export enum ActionType {
  ADD = "add",
  DELETE = "delete",
  UPDATE = "update",
}

export interface FileChange {
  type: ActionType;
  old_content?: string | null;
  new_content?: string | null;
  move_path?: string | null;
}

export interface Commit {
  changes: Record<string, FileChange>;
}

export function assemble_changes(
  orig: Record<string, string | null>,
  updatedFiles: Record<string, string | null>,
): Commit {
  const commit: Commit = { changes: {} };
  for (const [p, newContent] of Object.entries(updatedFiles)) {
    const oldContent = orig[p];
    if (oldContent === newContent) {
      continue;
    }
    if (oldContent !== undefined && newContent !== undefined) {
      commit.changes[p] = {
        type: ActionType.UPDATE,
        old_content: oldContent,
        new_content: newContent,
      };
    } else if (newContent !== undefined) {
      commit.changes[p] = {
        type: ActionType.ADD,
        new_content: newContent,
      };
    } else if (oldContent !== undefined) {
      commit.changes[p] = {
        type: ActionType.DELETE,
        old_content: oldContent,
      };
    } else {
      throw new Error("Unexpected state in assemble_changes");
    }
  }
  return commit;
}

// -----------------------------------------------------------------------------
// Patch‑related structures
// -----------------------------------------------------------------------------

export interface Chunk {
  orig_index: number; // line index of the first line in the original file
  del_lines: Array<string>;
  ins_lines: Array<string>;
}

export interface PatchAction {
  type: ActionType;
  new_file?: string | null;
  chunks: Array<Chunk>;
  move_path?: string | null;
}

export interface Patch {
  actions: Record<string, PatchAction>;
}

export class DiffError extends Error {}

// -----------------------------------------------------------------------------
// Parser (patch text -> Patch)
// -----------------------------------------------------------------------------

class Parser {
  current_files: Record<string, string>;
  lines: Array<string>;
  index = 0;
  patch: Patch = { actions: {} };
  fuzz = 0;

  constructor(currentFiles: Record<string, string>, lines: Array<string>) {
    this.current_files = currentFiles;
    this.lines = lines;
  }

  private is_done(prefixes?: Array<string>): boolean {
    if (this.index >= this.lines.length) {
      return true;
    }
    if (
      prefixes &&
      prefixes.some((p) => this.lines[this.index]!.startsWith(p.trim()))
    ) {
      return true;
    }
    return false;
  }

  private startswith(prefix: string | Array<string>): boolean {
    const prefixes = Array.isArray(prefix) ? prefix : [prefix];
    return prefixes.some((p) => this.lines[this.index]!.startsWith(p));
  }

  private read_str(prefix = "", returnEverything = false): string {
    if (this.index >= this.lines.length) {
      throw new DiffError(`Index: ${this.index} >= ${this.lines.length}`);
    }
    if (this.lines[this.index]!.startsWith(prefix)) {
      const text = returnEverything
        ? this.lines[this.index]
        : this.lines[this.index]!.slice(prefix.length);
      this.index += 1;
      return text ?? "";
    }
    return "";
  }

  parse(): void {
    while (!this.is_done([PATCH_SUFFIX])) {
      let path = this.read_str(UPDATE_FILE_PREFIX);
      if (path) {
        if (this.patch.actions[path]) {
          throw new DiffError(`Update File Error: Duplicate Path: ${path}`);
        }
        const moveTo = this.read_str(MOVE_FILE_TO_PREFIX);
        if (!(path in this.current_files)) {
          throw new DiffError(`Update File Error: Missing File: ${path}`);
        }
        const text = this.current_files[path];
        const action = this.parse_update_file(text ?? "");
        action.move_path = moveTo || undefined;
        this.patch.actions[path] = action;
        continue;
      }
      path = this.read_str(DELETE_FILE_PREFIX);
      if (path) {
        if (this.patch.actions[path]) {
          throw new DiffError(`Delete File Error: Duplicate Path: ${path}`);
        }
        if (!(path in this.current_files)) {
          throw new DiffError(`Delete File Error: Missing File: ${path}`);
        }
        this.patch.actions[path] = { type: ActionType.DELETE, chunks: [] };
        continue;
      }
      path = this.read_str(ADD_FILE_PREFIX);
      if (path) {
        if (this.patch.actions[path]) {
          throw new DiffError(`Add File Error: Duplicate Path: ${path}`);
        }
        if (path in this.current_files) {
          throw new DiffError(`Add File Error: File already exists: ${path}`);
        }
        this.patch.actions[path] = this.parse_add_file();
        continue;
      }
      throw new DiffError(`Unknown Line: ${this.lines[this.index]}`);
    }
    if (!this.startswith(PATCH_SUFFIX.trim())) {
      throw new DiffError("Missing End Patch");
    }
    this.index += 1;
  }

  private parse_update_file(text: string): PatchAction {
    const action: PatchAction = { type: ActionType.UPDATE, chunks: [] };
    const fileLines = text.split("\n");
    let index = 0;

    while (
      !this.is_done([
        PATCH_SUFFIX,
        UPDATE_FILE_PREFIX,
        DELETE_FILE_PREFIX,
        ADD_FILE_PREFIX,
        END_OF_FILE_PREFIX,
      ])
    ) {
      const defStr = this.read_str("@@ ");
      let sectionStr = "";
      if (!defStr && this.lines[this.index] === "@@") {
        sectionStr = this.lines[this.index]!;
        this.index += 1;
      }
      if (!(defStr || sectionStr || index === 0)) {
        throw new DiffError(`Invalid Line:\n${this.lines[this.index]}`);
      }
      if (defStr.trim()) {
        let found = false;
        // ------------------------------------------------------------------
        // Equality helpers using the canonicalisation from find_context_core.
        // (We duplicate a minimal version here because the scope is local.)
        // ------------------------------------------------------------------
        const canonLocal = (s: string): string =>
          s.normalize("NFC").replace(
            /./gu,
            (c) =>
              (
                ({
                  "-": "-",
                  "\u2010": "-",
                  "\u2011": "-",
                  "\u2012": "-",
                  "\u2013": "-",
                  "\u2014": "-",
                  "\u2212": "-",
                  "\u0022": '"',
                  "\u201C": '"',
                  "\u201D": '"',
                  "\u201E": '"',
                  "\u00AB": '"',
                  "\u00BB": '"',
                  "\u0027": "'",
                  "\u2018": "'",
                  "\u2019": "'",
                  "\u201B": "'",
                  "\u00A0": " ",
                  "\u202F": " ",
                }) as Record<string, string>
              )[c] ?? c,
          );

        if (
          !fileLines
            .slice(0, index)
            .some((s) => canonLocal(s) === canonLocal(defStr))
        ) {
          for (let i = index; i < fileLines.length; i++) {
            if (canonLocal(fileLines[i]!) === canonLocal(defStr)) {
              index = i + 1;
              found = true;
              break;
            }
          }
        }
        if (
          !found &&
          !fileLines
            .slice(0, index)
            .some((s) => canonLocal(s.trim()) === canonLocal(defStr.trim()))
        ) {
          for (let i = index; i < fileLines.length; i++) {
            if (
              canonLocal(fileLines[i]!.trim()) === canonLocal(defStr.trim())
            ) {
              index = i + 1;
              this.fuzz += 1;
              found = true;
              break;
            }
          }
        }
      }

      const [nextChunkContext, chunks, endPatchIndex, eof] = peek_next_section(
        this.lines,
        this.index,
      );
      const [newIndex, fuzz] = find_context(
        fileLines,
        nextChunkContext,
        index,
        eof,
      );
      if (newIndex === -1) {
        const ctxText = nextChunkContext.join("\n");
        if (eof) {
          throw new DiffError(`Invalid EOF Context ${index}:\n${ctxText}`);
        } else {
          throw new DiffError(`Invalid Context ${index}:\n${ctxText}`);
        }
      }
      this.fuzz += fuzz;
      for (const ch of chunks) {
        ch.orig_index += newIndex;
        action.chunks.push(ch);
      }
      index = newIndex + nextChunkContext.length;
      this.index = endPatchIndex;
    }
    return action;
  }

  private parse_add_file(): PatchAction {
    const lines: Array<string> = [];
    while (
      !this.is_done([
        PATCH_SUFFIX,
        UPDATE_FILE_PREFIX,
        DELETE_FILE_PREFIX,
        ADD_FILE_PREFIX,
      ])
    ) {
      const s = this.read_str();
      if (!s.startsWith(HUNK_ADD_LINE_PREFIX)) {
        throw new DiffError(`Invalid Add File Line: ${s}`);
      }
      lines.push(s.slice(1));
    }
    return {
      type: ActionType.ADD,
      new_file: lines.join("\n"),
      chunks: [],
    };
  }
}

function find_context_core(
  lines: Array<string>,
  context: Array<string>,
  start: number,
): [number, number] {
  // ---------------------------------------------------------------------------
  // Helpers – Unicode punctuation normalisation
  // ---------------------------------------------------------------------------

  /*
   * The patch-matching algorithm originally required **exact** string equality
   * for non-whitespace characters.  That breaks when the file on disk contains
   * visually identical but different Unicode code-points (e.g. “EN DASH” vs
   * ASCII "-"), because models almost always emit the ASCII variant.  To make
   * apply_patch resilient we canonicalise a handful of common punctuation
   * look-alikes before doing comparisons.
   *
   * We purposefully keep the mapping *small* – only characters that routinely
   * appear in source files and are highly unlikely to introduce ambiguity are
   * included.  Each entry is written using the corresponding Unicode escape so
   * that the file remains ASCII-only even after transpilation.
   */

  const PUNCT_EQUIV: Record<string, string> = {
    // Hyphen / dash variants --------------------------------------------------
    /* U+002D HYPHEN-MINUS */ "-": "-",
    /* U+2010 HYPHEN */ "\u2010": "-",
    /* U+2011 NO-BREAK HYPHEN */ "\u2011": "-",
    /* U+2012 FIGURE DASH */ "\u2012": "-",
    /* U+2013 EN DASH */ "\u2013": "-",
    /* U+2014 EM DASH */ "\u2014": "-",
    /* U+2212 MINUS SIGN */ "\u2212": "-",

    // Double quotes -----------------------------------------------------------
    /* U+0022 QUOTATION MARK */ "\u0022": '"',
    /* U+201C LEFT DOUBLE QUOTATION MARK */ "\u201C": '"',
    /* U+201D RIGHT DOUBLE QUOTATION MARK */ "\u201D": '"',
    /* U+201E DOUBLE LOW-9 QUOTATION MARK */ "\u201E": '"',
    /* U+00AB LEFT-POINTING DOUBLE ANGLE QUOTATION MARK */ "\u00AB": '"',
    /* U+00BB RIGHT-POINTING DOUBLE ANGLE QUOTATION MARK */ "\u00BB": '"',

    // Single quotes -----------------------------------------------------------
    /* U+0027 APOSTROPHE */ "\u0027": "'",
    /* U+2018 LEFT SINGLE QUOTATION MARK */ "\u2018": "'",
    /* U+2019 RIGHT SINGLE QUOTATION MARK */ "\u2019": "'",
    /* U+201B SINGLE HIGH-REVERSED-9 QUOTATION MARK */ "\u201B": "'",
    // Spaces ------------------------------------------------------------------
    /* U+00A0 NO-BREAK SPACE */ "\u00A0": " ",
    /* U+202F NARROW NO-BREAK SPACE */ "\u202F": " ",
  };

  const canon = (s: string): string =>
    s
      // Canonical Unicode composition first
      .normalize("NFC")
      // Replace punctuation look-alikes
      .replace(/./gu, (c) => PUNCT_EQUIV[c] ?? c);
  if (context.length === 0) {
    return [start, 0];
  }
  // Pass 1 – exact equality after canonicalisation ---------------------------
  const canonicalContext = canon(context.join("\n"));
  for (let i = start; i < lines.length; i++) {
    const segment = canon(lines.slice(i, i + context.length).join("\n"));
    if (segment === canonicalContext) {
      return [i, 0];
    }
  }

  // Pass 2 – ignore trailing whitespace -------------------------------------
  for (let i = start; i < lines.length; i++) {
    const segment = canon(
      lines
        .slice(i, i + context.length)
        .map((s) => s.trimEnd())
        .join("\n"),
    );
    const ctx = canon(context.map((s) => s.trimEnd()).join("\n"));
    if (segment === ctx) {
      return [i, 1];
    }
  }

  // Pass 3 – ignore all surrounding whitespace ------------------------------
  for (let i = start; i < lines.length; i++) {
    const segment = canon(
      lines
        .slice(i, i + context.length)
        .map((s) => s.trim())
        .join("\n"),
    );
    const ctx = canon(context.map((s) => s.trim()).join("\n"));
    if (segment === ctx) {
      return [i, 100];
    }
  }

  return [-1, 0];
}

function find_context(
  lines: Array<string>,
  context: Array<string>,
  start: number,
  eof: boolean,
): [number, number] {
  if (eof) {
    let [newIndex, fuzz] = find_context_core(
      lines,
      context,
      lines.length - context.length,
    );
    if (newIndex !== -1) {
      return [newIndex, fuzz];
    }
    [newIndex, fuzz] = find_context_core(lines, context, start);
    return [newIndex, fuzz + 10000];
  }
  return find_context_core(lines, context, start);
}

function peek_next_section(
  lines: Array<string>,
  initialIndex: number,
): [Array<string>, Array<Chunk>, number, boolean] {
  let index = initialIndex;
  const old: Array<string> = [];
  let delLines: Array<string> = [];
  let insLines: Array<string> = [];
  const chunks: Array<Chunk> = [];
  let mode: "keep" | "add" | "delete" = "keep";

  while (index < lines.length) {
    const s = lines[index]!;
    if (
      [
        "@@",
        PATCH_SUFFIX,
        UPDATE_FILE_PREFIX,
        DELETE_FILE_PREFIX,
        ADD_FILE_PREFIX,
        END_OF_FILE_PREFIX,
      ].some((p) => s.startsWith(p.trim()))
    ) {
      break;
    }
    if (s === "***") {
      break;
    }
    if (s.startsWith("***")) {
      throw new DiffError(`Invalid Line: ${s}`);
    }
    index += 1;
    const lastMode: "keep" | "add" | "delete" = mode;
    let line = s;
    if (line[0] === HUNK_ADD_LINE_PREFIX) {
      mode = "add";
    } else if (line[0] === "-") {
      mode = "delete";
    } else if (line[0] === " ") {
      mode = "keep";
    } else {
      // Tolerate invalid lines where the leading whitespace is missing. This is necessary as
      // the model sometimes doesn't fully adhere to the spec and returns lines without leading
      // whitespace for context lines.
      mode = "keep";
      line = " " + line;

      // TODO: Re-enable strict mode.
      // throw new DiffError(`Invalid Line: ${line}`)
    }

    line = line.slice(1);
    if (mode === "keep" && lastMode !== mode) {
      if (insLines.length || delLines.length) {
        chunks.push({
          orig_index: old.length - delLines.length,
          del_lines: delLines,
          ins_lines: insLines,
        });
      }
      delLines = [];
      insLines = [];
    }
    if (mode === "delete") {
      delLines.push(line);
      old.push(line);
    } else if (mode === "add") {
      insLines.push(line);
    } else {
      old.push(line);
    }
  }
  if (insLines.length || delLines.length) {
    chunks.push({
      orig_index: old.length - delLines.length,
      del_lines: delLines,
      ins_lines: insLines,
    });
  }
  if (index < lines.length && lines[index] === END_OF_FILE_PREFIX) {
    index += 1;
    return [old, chunks, index, true];
  }
  return [old, chunks, index, false];
}

// -----------------------------------------------------------------------------
// High‑level helpers
// -----------------------------------------------------------------------------

export function text_to_patch(
  text: string,
  orig: Record<string, string>,
): [Patch, number] {
  const lines = text.trim().split("\n");
  if (
    lines.length < 2 ||
    !(lines[0] ?? "").startsWith(PATCH_PREFIX.trim()) ||
    lines[lines.length - 1] !== PATCH_SUFFIX.trim()
  ) {
    let reason = "Invalid patch text: ";
    if (lines.length < 2) {
      reason += "Patch text must have at least two lines.";
    } else if (!(lines[0] ?? "").startsWith(PATCH_PREFIX.trim())) {
      reason += "Patch text must start with the correct patch prefix.";
    } else if (lines[lines.length - 1] !== PATCH_SUFFIX.trim()) {
      reason += "Patch text must end with the correct patch suffix.";
    }
    throw new DiffError(reason);
  }
  const parser = new Parser(orig, lines);
  parser.index = 1;
  parser.parse();
  return [parser.patch, parser.fuzz];
}

export function identify_files_needed(text: string): Array<string> {
  const lines = text.trim().split("\n");
  const result = new Set<string>();
  for (const line of lines) {
    if (line.startsWith(UPDATE_FILE_PREFIX)) {
      result.add(line.slice(UPDATE_FILE_PREFIX.length));
    }
    if (line.startsWith(DELETE_FILE_PREFIX)) {
      result.add(line.slice(DELETE_FILE_PREFIX.length));
    }
  }
  return [...result];
}

export function identify_files_added(text: string): Array<string> {
  const lines = text.trim().split("\n");
  const result = new Set<string>();
  for (const line of lines) {
    if (line.startsWith(ADD_FILE_PREFIX)) {
      result.add(line.slice(ADD_FILE_PREFIX.length));
    }
  }
  return [...result];
}

function _get_updated_file(
  text: string,
  action: PatchAction,
  path: string,
): string {
  if (action.type !== ActionType.UPDATE) {
    throw new Error("Expected UPDATE action");
  }
  const origLines = text.split("\n");
  const destLines: Array<string> = [];
  let origIndex = 0;
  for (const chunk of action.chunks) {
    if (chunk.orig_index > origLines.length) {
      throw new DiffError(
        `${path}: chunk.orig_index ${chunk.orig_index} > len(lines) ${origLines.length}`,
      );
    }
    if (origIndex > chunk.orig_index) {
      throw new DiffError(
        `${path}: orig_index ${origIndex} > chunk.orig_index ${chunk.orig_index}`,
      );
    }
    destLines.push(...origLines.slice(origIndex, chunk.orig_index));
    const delta = chunk.orig_index - origIndex;
    origIndex += delta;

    // inserted lines
    if (chunk.ins_lines.length) {
      for (const l of chunk.ins_lines) {
        destLines.push(l);
      }
    }
    origIndex += chunk.del_lines.length;
  }
  destLines.push(...origLines.slice(origIndex));
  return destLines.join("\n");
}

export function patch_to_commit(
  patch: Patch,
  orig: Record<string, string>,
): Commit {
  const commit: Commit = { changes: {} };
  for (const [pathKey, action] of Object.entries(patch.actions)) {
    if (action.type === ActionType.DELETE) {
      commit.changes[pathKey] = {
        type: ActionType.DELETE,
        old_content: orig[pathKey],
      };
    } else if (action.type === ActionType.ADD) {
      commit.changes[pathKey] = {
        type: ActionType.ADD,
        new_content: action.new_file ?? "",
      };
    } else if (action.type === ActionType.UPDATE) {
      const newContent = _get_updated_file(orig[pathKey]!, action, pathKey);
      commit.changes[pathKey] = {
        type: ActionType.UPDATE,
        old_content: orig[pathKey],
        new_content: newContent,
        move_path: action.move_path ?? undefined,
      };
    }
  }
  return commit;
}

// -----------------------------------------------------------------------------
// Filesystem helpers for Node environment
// -----------------------------------------------------------------------------

export function load_files(
  paths: Array<string>,
  openFn: (p: string) => string,
): Record<string, string> {
  const orig: Record<string, string> = {};
  for (const p of paths) {
    try {
      orig[p] = openFn(p);
    } catch {
      // Convert any file read error into a DiffError so that callers
      // consistently receive DiffError for patch-related failures.
      throw new DiffError(`File not found: ${p}`);
    }
  }
  return orig;
}

export function apply_commit(
  commit: Commit,
  writeFn: (p: string, c: string) => void,
  removeFn: (p: string) => void,
): void {
  for (const [p, change] of Object.entries(commit.changes)) {
    if (change.type === ActionType.DELETE) {
      removeFn(p);
    } else if (change.type === ActionType.ADD) {
      writeFn(p, change.new_content ?? "");
    } else if (change.type === ActionType.UPDATE) {
      if (change.move_path) {
        writeFn(change.move_path, change.new_content ?? "");
        removeFn(p);
      } else {
        writeFn(p, change.new_content ?? "");
      }
    }
  }
}

export function process_patch(
  text: string,
  openFn: (p: string) => string,
  writeFn: (p: string, c: string) => void,
  removeFn: (p: string) => void,
): string {
  if (!text.startsWith(PATCH_PREFIX)) {
    throw new DiffError("Patch must start with *** Begin Patch\\n");
  }
  const paths = identify_files_needed(text);
  const orig = load_files(paths, openFn);
  const [patch, _fuzz] = text_to_patch(text, orig);
  const commit = patch_to_commit(patch, orig);
  apply_commit(commit, writeFn, removeFn);
  return "Done!";
}

// -----------------------------------------------------------------------------
// Default filesystem implementations
// -----------------------------------------------------------------------------

function open_file(p: string): string {
  return fs.readFileSync(p, "utf8");
}

function write_file(p: string, content: string): void {
  if (path.isAbsolute(p)) {
    throw new DiffError("We do not support absolute paths.");
  }
  const parent = path.dirname(p);
  if (parent !== ".") {
    fs.mkdirSync(parent, { recursive: true });
  }
  fs.writeFileSync(p, content, "utf8");
}

function remove_file(p: string): void {
  fs.unlinkSync(p);
}

// -----------------------------------------------------------------------------
// CLI mode. Not exported, executed only if run directly.
// -----------------------------------------------------------------------------

if (import.meta.url === `file://${process.argv[1]}`) {
  let patchText = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => (patchText += chunk));
  process.stdin.on("end", () => {
    if (!patchText) {
      // eslint-disable-next-line no-console
      console.error("Please pass patch text through stdin");
      process.exit(1);
    }
    try {
      const result = process_patch(
        patchText,
        open_file,
        write_file,
        remove_file,
      );
      // eslint-disable-next-line no-console
      console.log(result);
    } catch (err: unknown) {
      // eslint-disable-next-line no-console
      console.error(err instanceof Error ? err.message : String(err));
      process.exit(1);
    }
  });
}

export const applyPatchToolInstructions = `
To edit files, ALWAYS use the \`shell\` tool with \`apply_patch\` CLI.  \`apply_patch\` effectively allows you to execute a diff/patch against a file, but the format of the diff specification is unique to this task, so pay careful attention to these instructions. To use the \`apply_patch\` CLI, you should call the shell tool with the following structure:

\`\`\`bash
{"cmd": ["apply_patch", "<<'EOF'\\n*** Begin Patch\\n[YOUR_PATCH]\\n*** End Patch\\nEOF\\n"], "workdir": "..."}
\`\`\`

Where [YOUR_PATCH] is the actual content of your patch, specified in the following V4A diff format.

*** [ACTION] File: [path/to/file] -> ACTION can be one of Add, Update, or Delete.
For each snippet of code that needs to be changed, repeat the following:
[context_before] -> See below for further instructions on context.
- [old_code] -> Precede the old code with a minus sign.
+ [new_code] -> Precede the new, replacement code with a plus sign.
[context_after] -> See below for further instructions on context.

For instructions on [context_before] and [context_after]:
- By default, show 3 lines of code immediately above and 3 lines immediately below each change. If a change is within 3 lines of a previous change, do NOT duplicate the first change’s [context_after] lines in the second change’s [context_before] lines.
- If 3 lines of context is insufficient to uniquely identify the snippet of code within the file, use the @@ operator to indicate the class or function to which the snippet belongs. For instance, we might have:
@@ class BaseClass
[3 lines of pre-context]
- [old_code]
+ [new_code]
[3 lines of post-context]

- If a code block is repeated so many times in a class or function such that even a single \`@@\` statement and 3 lines of context cannot uniquely identify the snippet of code, you can use multiple \`@@\` statements to jump to the right context. For instance:

@@ class BaseClass
@@ 	def method():
[3 lines of pre-context]
- [old_code]
+ [new_code]
[3 lines of post-context]

Note, then, that we do not use line numbers in this diff format, as the context is enough to uniquely identify code. An example of a message that you might pass as "input" to this function, in order to apply a patch, is shown below.

\`\`\`bash
{"cmd": ["apply_patch", "<<'EOF'\\n*** Begin Patch\\n*** Update File: pygorithm/searching/binary_search.py\\n@@ class BaseClass\\n@@     def search():\\n-        pass\\n+        raise NotImplementedError()\\n@@ class Subclass\\n@@     def search():\\n-        pass\\n+        raise NotImplementedError()\\n*** End Patch\\nEOF\\n"], "workdir": "..."}
\`\`\`

File references can only be relative, NEVER ABSOLUTE. After the apply_patch command is run, it will always say "Done!", regardless of whether the patch was successfully applied or not. However, you can determine if there are issue and errors by looking at any warnings or logging lines printed BEFORE the "Done!" is output.
`;
