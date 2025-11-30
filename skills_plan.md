Skills Injection Design Doc
===========================

Context and motivation
----------------------
Codex currently has no notion of reusable “skills” that can be discovered on disk and injected into the runtime context. We want to mirror the productive parts of other prior art in this space while keeping the first iteration deliberately small and safe. 

Goals:
- Act as an extension of the existing agents.md mechanism, which allows progressive disclosure of content and instructions driven by the agent's own decision making process.
- Reduce repeated prompting by letting users package domain expertise once.

Non-goals for first implementation (v1): tool restrictions, hot reload, network/package installation, hierarchies of skills with their own progressive disclosure or supporting skills defined elsewhere than the user home root.

Implementation considerations:
- Keep the runtime context lean by injecting only lightweight metadata (name, description, path) and not inlining full instructions.
- Avoid mutating source files; the feature should be entirely runtime-driven and reversible.
- Establish a structure that can grow to project-local skills later (e.g., alongside agents.md in a workspace).
- Provide strong validation and clear failure modes so misconfigured skills do not silently degrade agent behavior.

High-level behavior
-------------------
- On startup, codex discovers skills from the user home root `~/.codex/skills`, recursively.
- Each skill lives in a directory containing a file named exactly `SKILL.md` with YAML frontmatter (`name`, `description`) and a Markdown body. Extra keys are allowed and ignored.
- Valid skills are rendered into the agent context by dynamically appending a `## Skills` section to the in-memory content of the root `agents.md`. The file on disk remains unchanged.
- The `## Skills` section contains one-line bullets per skill: name, description, and absolute path to the `SKILL.md`, plus a brief intro paragraph explaining usage. No skill body content is inlined.
- Invalid skills (parse errors, missing required fields, overlength fields) block startup with a dismissible modal that lists every invalid path and error. Errors are also logged. After dismissal, invalid skills are ignored for rendering. The modal reappears on subsequent startups until the issues are fixed.
- If no valid skills are found, the Skills section is omitted entirely.
- Loading occurs once at startup; no hot reload in v1.

Discovery rules
---------------
- Roots: only `~/.codex/skills` in v1. Design the loader so additional roots can be added later (e.g., `<folder>/skills` alongside `<folder>/agents.md` for project-scoped skills).
- Recursion: traverse all subdirectories.
- Hidden entries: skip hidden directories and files (prefix “.”).
- Symlinks: do not follow symlinks.
- Recognition: only files named exactly `SKILL.md` qualify as skills.
- Ordering: collect all skills (no dedupe), then sort by `name`, then by absolute path for stable rendering.

Skill format and validation
---------------------------
- File: `SKILL.md` must start with YAML frontmatter delimited by `---` on its own lines; the next `---` ends the frontmatter. The remainder is body (ignored for rendering).
- Required fields: `name` (string), `description` (string).
- Length constraints: `name` non-empty, max 100 chars; `description` non-empty, max 500 chars. Enforce hard errors if exceeded.
- Sanitization for rendering: trim, collapse newlines/tabs/extra whitespace inside `name` and `description` to single spaces to preserve single-line output.
- Extra keys: allowed and ignored for forward compatibility.
- YAML parsing: use serde_yaml; accept CRLF; any parse error or missing required field is a hard error.

Rendering model
---------------
- Base content: read the root `agents.md` (do not mutate it).
- Synthetic section: append to the in-memory content a final section only if there is at least one valid skill.
  - Heading: `## Skills`
  - Intro paragraph (single line), suggested copy: “These skills are discovered at startup from ~/.codex/skills; each entry shows name, description, and file path so you can open the source for full instructions. Content is not inlined to keep context lean.”
  - Entries: one bullet per skill, single line, format `- <name>: <description> (file: /absolute/path/to/SKILL.md)`. Use sanitized fields to avoid line breaks.
  - Paths: always absolute for clarity.
- If no valid skills: omit the section entirely.

Error handling and UX
---------------------
- Invalid skills: any validation or parse failure is treated as blocking. On startup, present a dismissible modal that lists every invalid skill with its path and detailed error message. Startup is paused until the user dismisses.
- Persistence: the modal reappears on every startup until all invalid skills are fixed or removed.
- Logging: also emit error-level logs for each invalid skill, matching what is shown in the modal.
- Post-dismissal behavior: invalid skills are ignored for rendering; valid skills still render.
- Reuse existing modal/popup pattern in the codebase for consistency; add a button to dismiss/continue.

Update cadence
--------------
- Load skills once at startup. No hot-reload, no file watching. Users must restart codex for changes to take effect.

Future-proofing
---------------
- Multiple roots: structure the loader to accept a list of roots (initially one: `~/.codex/skills`). Future additions: `<workspace>/skills` co-located with `<workspace>/agents.md`, or other configurable roots.
- Rendering target: keep the renderer generic—given base agents.md content and a skill list, return augmented content. This will let us plug in different agents.md locations or multiple agents files later.
- Allowed-tools: not implemented in v1, but the validator should ignore extra keys so later fields can be added without breaking old skills.
- Potential later enhancements: hot reload; per-project skills; UI for listing valid skills; richer summaries; optional inline previews under a size threshold; deduplication policies.

Implementation outline
----------------------
- Discovery:
  - Expand `~` to absolute path.
  - Walk directories recursively; skip hidden entries; do not follow symlinks.
  - Collect paths exactly matching `SKILL.md`.
- Parsing/validation:
  - For each path, read file; locate frontmatter between `---` lines.
  - Parse with serde_yaml into a struct { name: String, description: String, …ignored }.
  - Trim and sanitize; enforce non-empty and length limits.
  - On failure, record an error entry (path + message).
- Rendering:
  - Sort valid skills by name, then path.
  - If none, return base agents.md unchanged.
  - Otherwise, append the section with intro and bullets, ensuring one-line entries.
- Error surfacing:
  - If any errors exist, build a blocking modal view at startup listing all (path + message), with a dismiss/continue button. Use existing modal screen as template.
  - Log all errors via the standard logger at error level.
  - After dismissal, proceed with rendering valid skills only.

Testing approach
----------------
- Unit tests:
  - Frontmatter parsing and validation (missing fields, overlength, malformed YAML).
  - Sanitization (newline/tab collapsing).
  - Ordering and rendering format.
  - Hidden/ symlink skipping.
- Integration tests:
  - With a temp `~/.codex/skills` tree containing multiple valid and invalid skills, verify augmented agents content and that invalid ones are omitted.
  - Modal trigger: simulate startup with invalid skills and assert the error list is produced (if test harness supports UI assertions).
- Manual checks:
  - Create valid/invalid SKILLs under `~/.codex/skills`, start codex, observe modal and final context.
  - Confirm agents.md on disk remains unchanged.

Risks and mitigations
---------------------
- Risk: Excessively long descriptions breaking layout. Mitigation: hard length limits and sanitization.
- Risk: Users surprised by blocking modal. Mitigation: clear messaging, dismiss to proceed, repeat only until fixed.
- Risk: Future roots complicate ordering. Mitigation: keep explicit ordering rules (name, then path) and stable formatting.
- Risk: Context bloat if too many skills. Mitigation: metadata-only render, short descriptions, length caps; consider future pagination or caps if needed.

Acceptance criteria
-------------------
- With valid skills present, the runtime context includes a final `## Skills` section with the intro and one-line bullets (name, description, absolute path), sorted by name then path. agents.md on disk is untouched.
- If no skills are present, no Skills section is injected.
- Any invalid skill causes a blocking, dismissible startup modal listing all invalid paths and errors; errors also logged; invalid skills are excluded from rendering. The modal reappears on subsequent startups until resolved.

Mermaid data flow (current + future roots)
------------------------------------------

```mermaid
flowchart TD
    subgraph Current_v1["Current (v1)"]
        skills_dir["~/.codex/skills/**/SKILL.md (skip hidden/symlinks)"]
        load[load_skills -> validate + sanitize -> SkillMetadata]
        render[render_skills_section -> \"## Skills\\n- name: desc (file: ...)\"]
        agents[read_project_docs(agents.md)]
        merge[merge agents.md + Skills section\n(runtime only, no disk changes)]
        skills_dir --> load --> render --> merge
        agents --> merge --> final_v1["runtime instructions"]
    end

    subgraph Future_multi_root["Future (multi-root)"]
        roots["Roots: ~/.codex/skills; <repo>/skills next to agents.md; nested crate skills"]
        per_root_discover["per-root: discover skills (recursive, same rules)"]
        per_root_validate["per-root: validate -> SkillMetadata"]
        per_root_render["per-root: render optional Skills section"]
        per_agents["per agents.md: read content, append its Skills section"]
        concat["concatenate from repo root -> cwd"]
        final_future["runtime instructions\n= user_instructions? + \"--- project-doc ---\" + concatenated agents+skills"]
        roots --> per_root_discover --> per_root_validate --> per_root_render --> per_agents --> concat --> final_future
    end
```
