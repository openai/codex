# Skills (experimental)

> **Warning:** This is an experimental and non-stable feature. If you depend on it, please expect breaking changes over the coming weeks and understand that there is currently no guarantee that this works well. Use at your own risk!

Codexel can automatically discover reusable "skills" you keep on disk. A skill is a small bundle with a name, a short description (what it does and when to use it), and an optional body of instructions you can open when needed. Codexel injects only the name, description, and file path into the runtime context; the body stays on disk.

## Enable skills

Skills are behind the experimental `skills` feature flag and are disabled by default.

- Enable in config (preferred): add the following to `$CODEXEL_HOME/config.toml` (usually `~/.codexel/config.toml`, or legacy `~/.codex/config.toml`) and restart Codexel:

  ```toml
  [features]
  skills = true
  ```

- Enable for a single run: launch Codexel with `codexel --enable skills`

## Where skills live

- Location (v1): `~/.codexel/skills/**/SKILL.md` (recursive). Hidden entries and symlinks are skipped. Only files named exactly `SKILL.md` count.
- Sorting: rendered by name, then path for stability.

## File format

- YAML frontmatter + body.
  - Required:
    - `name` (non-empty, ≤100 chars, sanitized to one line)
    - `description` (non-empty, ≤500 chars, sanitized to one line)
  - Extra keys are ignored. The body can contain any Markdown; it is not injected into context.

## Loading and rendering

- Loaded once at startup.
- If valid skills exist, Codexel appends a runtime-only `## Skills` section after `AGENTS.md`, one bullet per skill: `- <name>: <description> (file: /absolute/path/to/SKILL.md)`.
- If no valid skills exist, the section is omitted. On-disk files are never modified.

## Using skills

- Mention a skill by name in a message using `$<skill-name>`.
- In the TUI, you can also use `/skills` to browse and insert skills.

## Validation and errors

- Invalid skills (missing/invalid YAML, empty/over-length fields) trigger a blocking, dismissible startup modal in the TUI that lists each path and error. Errors are also logged. You can dismiss to continue (invalid skills are ignored) or exit. Fix SKILL.md files and restart to clear the modal.

## Create a skill

1. Create `~/.codexel/skills/<skill-name>/`.
2. Add `SKILL.md`:

   ```
   ---
   name: your-skill-name
   description: what it does and when to use it (<=500 chars)
   ---

   # Optional body
   Add instructions, references, examples, or scripts (kept on disk).
   ```

3. Keep `name`/`description` within the limits; avoid newlines in those fields.
4. Restart Codexel to load the new skill.

## Example

```
mkdir -p ~/.codexel/skills/pdf-processing
cat <<'SKILL_EXAMPLE' > ~/.codexel/skills/pdf-processing/SKILL.md
---
name: pdf-processing
description: Extract text and tables from PDFs; use when PDFs, forms, or document extraction are mentioned.
---

# PDF Processing
- Use pdfplumber to extract text.
- For form filling, see FORMS.md.
SKILL_EXAMPLE
```
