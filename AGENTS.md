IMPORTANT: **THINK** carefully and analyze all points of view.

When in doubt, **RESEARCH** or **ASK**.

The cardinal rule you must follow is: **NEVER** write code haphazardly with only the final result in mind. The final result is the CONSEQUENCE of code written with excellence, robustness, and elegance.
**NEVER** do anything in a hurry; haste is the enemy of perfection. Take the time you need to write perfect code.

Whenever you propose or implement a solution, **DO NOT REINVENT THE WHEEL**. Fix root causes; do not rely on quick fixes, hacks, or shit workarounds.

Prioritize error handling instead of fallbacks.
Avoid generic helpers and redundant, unnecessary validations.
Be thorough with verbose output and debugging.
In Python scripts, include progress bars when appropriate.
Only change variable and function names when STRICTLY necessary.
Robust code BUT without frills.
Use descriptive, intelligible variable and function names.

⚠️ IMPORTANT: **DO NOT** use git clean under any circumstances.

# Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Crate names are prefixed with `codex-`. For example, the `core` folder's crate is named `codex-core`
- When using format! and you can inline variables into {}, always do that.
- Install any commands the repo relies on (for example `just`, `rg`, or `cargo-insta`) if they aren't already available before running instructions here.
- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
  - You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.
  - Similarly, when you spawn a process using Seatbelt (`/usr/bin/sandbox-exec`), `CODEX_SANDBOX=seatbelt` will be set on the child process. Integration tests that want to run Seatbelt themselves cannot be run under Seatbelt, so checks for `CODEX_SANDBOX=seatbelt` are also often used to early exit out of tests, as appropriate.
- Always collapse if statements per https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
- Always inline format! args when possible per https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
- Use method references over closures when possible per https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls
- When writing tests, prefer comparing the equality of entire objects over fields one by one.

Run `just fmt` (in `codex-rs` directory) automatically after making Rust code changes; do not ask for approval to run it. Before finalizing a change to `codex-rs`, run `just fix -p <project>` (in `codex-rs` directory) to fix any linter issues in the code. Prefer scoping with `-p` to avoid slow workspace‑wide Clippy builds; only run `just fix` without `-p` if you changed shared crates. Additionally, run the tests:

1. Run the test for the specific project that was changed. For example, if changes were made in `codex-rs/tui`, run `cargo test -p codex-tui`.
2. Once those pass, if any changes were made in common, core, or protocol, run the complete test suite with `cargo test --all-features`.
   When running interactively, ask the user before running `just fix` to finalize. `just fmt` does not require approval. project-specific or individual tests can be run without asking the user, but do ask the user before running the complete test suite.

## TUI style conventions

See `codex-rs/tui/styles.md`.

## TUI code conventions

- Use concise styling helpers from ratatui’s Stylize trait.
  - Basic spans: use "text".into()
  - Styled spans: use "text".red(), "text".green(), "text".magenta(), "text".dim(), etc.
  - Prefer these over constructing styles with `Span::styled` and `Style` directly.
  - Example: patch summary file lines
    - Desired: vec!["  └ ".into(), "M".red(), " ".dim(), "tui/src/app.rs".dim()]

### TUI Styling (ratatui)

- Prefer Stylize helpers: use "text".dim(), .bold(), .cyan(), .italic(), .underlined() instead of manual Style where possible.
- Prefer simple conversions: use "text".into() for spans and vec![…].into() for lines; when inference is ambiguous (e.g., Paragraph::new/Cell::from), use Line::from(spans) or Span::from(text).
- Computed styles: if the Style is computed at runtime, using `Span::styled` is OK (`Span::from(text).set_style(style)` is also acceptable).
- Avoid hardcoded white: do not use `.white()`; prefer the default foreground (no color).
- Chaining: combine helpers by chaining for readability (e.g., url.cyan().underlined()).
- Single items: prefer "text".into(); use Line::from(text) or Span::from(text) only when the target type isn’t obvious from context, or when using .into() would require extra type annotations.
- Building lines: use vec![…].into() to construct a Line when the target type is obvious and no extra type annotations are needed; otherwise use Line::from(vec![…]).
- Avoid churn: don’t refactor between equivalent forms (Span::styled ↔ set_style, Line::from ↔ .into()) without a clear readability or functional gain; follow file‑local conventions and do not introduce type annotations solely to satisfy .into().
- Compactness: prefer the form that stays on one line after rustfmt; if only one of Line::from(vec![…]) or vec![…].into() avoids wrapping, choose that. If both wrap, pick the one with fewer wrapped lines.

### Text wrapping

- Always use textwrap::wrap to wrap plain strings.
- If you have a ratatui Line and you want to wrap it, use the helpers in tui/src/wrapping.rs, e.g. word_wrap_lines / word_wrap_line.
- If you need to indent wrapped lines, use the initial_indent / subsequent_indent options from RtOptions if you can, rather than writing custom logic.
- If you have a list of lines and you need to prefix them all with some prefix (optionally different on the first vs subsequent lines), use the `prefix_lines` helper from line_utils.

## Tests

### Snapshot tests

This repo uses snapshot tests (via `insta`), especially in `codex-rs/tui`, to validate rendered output. When UI or text output changes intentionally, update the snapshots as follows:

- Run tests to generate any updated snapshots:
  - `cargo test -p codex-tui`
- Check what’s pending:
  - `cargo insta pending-snapshots -p codex-tui`
- Review changes by reading the generated `*.snap.new` files directly in the repo, or preview a specific file:
  - `cargo insta show -p codex-tui path/to/file.snap.new`
- Only if you intend to accept all new snapshots in this crate, run:
  - `cargo insta accept -p codex-tui`

If you don’t have the tool:

- `cargo install cargo-insta`

### Test assertions

- Tests should use pretty_assertions::assert_eq for clearer diffs. Import this at the top of the test module if it isn't already.

## Upstream Comparison & Integration

Use this repeatable flow whenever you are bringing in new changes or preparing a PR‑sized modification. The goal is to catch style drift, compile‑time issues (lifetimes, ownership, visibility), and snapshot deltas early.

### 1) Prepare remotes and fetch

- Ensure remotes:
  - `git remote -v` → must show `origin` and `upstream`
- Fetch and prune:
  - `git fetch upstream --prune`

### 2) Inspect diffs at commit and file level

- Commits not in upstream:
  - `git log --oneline upstream/main..main`
- Commits missing locally:
  - `git log --oneline main..upstream/main`
- File status by area (Rust workspace):
  - `git diff --name-status upstream/main..HEAD -- codex-rs`
- Deep diff for specific crates:
  - `git diff upstream/main..HEAD -- codex-rs/{tui,core,protocol}/src`

Focus during review:
- Public/private boundaries: avoid referring to private submodules across crates; re‑export symbols from `mod.rs` when needed.
- Lifetimes & ownership: prefer owned strings for cached UI props; pass `&Props` to pure readers; avoid moving out of shared refs.
- Type inference: do not stack conversions (`.into().into()`); `push_span(Into<Span>)` does not need extra `.into()` on inputs already returning `Span`.
- ratatui conventions: use `Stylize`, `Line`/`Span`, `prefix_lines`, and existing wrapping helpers; keep helpers small and composable.
- Error class to scan for: E0521/E0507/E0382/E0603/E0283 (lifetimes, moves, borrow escaping, visibility, trait inference).

### 3) Build, lint, and run scoped tests

- Format + clippy (scoped):
  - `cd codex-rs && just fmt`
  - `just fix -p <crate>` (ex.: `codex-tui`, `codex-core`)
- Run tests for the crate que mudou:
  - `cargo test -p <crate>`
- Se tocar `common`, `core` ou `protocol`, valide conjunto:
  - `cargo test --all-features`

### 4) Snapshot testing (TUI)

- Gerar diferenças:
  - `cargo test -p codex-tui`
- List pending:
  - `cargo insta pending-snapshots -p codex-tui`
- Inspecionar/aceitar:
  - `cargo insta show -p codex-tui path/to/file.snap.new` (opcional)
  - `cargo insta accept -p codex-tui` (somente se a mudança de UI for intencional)
- Commitar apenas código + arquivos `*.snap` alterados.

### 5) Commit and push hygiene

- Do not commit toolchains/caches.
  - `.gitignore` already includes: `codex-rs/.cargo/`, `codex-rs/.cargo-home/`, `codex-rs/.rustup/`, common image extensions, and `stash_*.rs`.
  - If a large artifact was staged by mistake:
    - `git reset --soft origin/main`
    - `git restore --staged .`
    - `git add <code changes> <*.snap> .gitignore`
    - `git commit -m "..." && git push`

## Change Review Guidelines (genéricas)

- Assinatura de helpers: prefira `fn f(props: &Props)` a `fn f(props: Props)` quando só lê; isso evita cópias/moves acidentais e facilita reuso.
- Propriedades persistidas de UI: use `String` ou `Cow<'static, str>` quando o valor é cacheado além do escopo atual.
- Reexports: quando tipos utilitários são usados fora do módulo, reexporte em `mod.rs` (`pub(crate) use ...`) em vez de referenciar submódulo privado.
- Testes de snapshot: só aceite quando a mudança de UI for intencional; mantenha as strings estáveis e ASCII para minimizar diffs.
- Visual consistency: reuse existing separators and textual style; avoid hard‑coded white; prefer `.dim()` for metadata.
## Repo Hygiene (git)

- Do not create new branches unless explicitly instructed by the user. Do all work directly on `main` by default.
- Never push or open PRs from feature/fix branches without explicit approval.
- When comparing or integrating remote work, use the steps in “Upstream Comparison & Integration” without creating local branches.

- Do not commit local toolchains or caches. Ensure these are in `.gitignore` (already present):
  - `codex-rs/.cargo/`, `codex-rs/.cargo-home/`, `codex-rs/.rustup/`
  - Image assets: `*.png`, `*.jpg`, `*.jpeg`, `*.gif`, `*.bmp`, `*.tiff`, `*.webp`, `*.svg`
  - Local drafts: `stash_*.rs`

- Seeding a clean commit after an accidental add of caches:
  - `git reset --soft origin/main`
  - `git restore --staged .`
  - `git add <code changes> <updated *.snap> .gitignore`
  - `git commit -m "..." && git push`

## Quick checklist (footer/status changes)

// Use this generic checklist when integrating UI/status changes:
- [ ] No private module leaks (re‑export when needed)
- [ ] No `'static` requirements for cached UI data unless necessary
- [ ] Helper functions take `&Props` when they only read
- [ ] Snapshot diffs reviewed and accepted intentionally
- [ ] Only code + `*.snap` changes committed
- [ ] Ctrl‑C / Esc: `context left | <hint>`
- [ ] Separadores ` | ` e ` · ` (antes de `? for shortcuts`)
- [ ] Atualizar snapshots do TUI (aceitar `*.snap`) 
