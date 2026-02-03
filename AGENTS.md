# codez

## Git worktree 運用（`.worktrees/`）

- ワークツリーはリポジトリ直下の `.worktrees/<name>` に作る（誤コミット防止のため `.gitignore` 済み）。
- 作成:
  - `git worktree add -b <name> .worktrees/<name> <start-point>`
- 削除（チェックアウトのみ削除。ブランチは消さない）:
  - `git worktree remove .worktrees/<name>`
- 一覧: `git worktree list`
- 注意:
  - `git clean -fdx` / `git clean -fdX` は **`.worktrees/` を消し得る**（運用上、基本使わない）。
  - 破棄済み worktree の参照整理は `git worktree prune`。

# Rust/codex-rs

## VSCode拡張のバージョン運用

- `vscode-extension/package.json` の `version` はローカル開発（`pnpm -C vscode-extension vsix:install`）では変更しない。
- バージョンを上げるのはユーザーが明示的に Publish を指示したタイミングのみ。
- 運用は「Publish時に1つ上げる → そのバージョンで開発/修正を続ける → 次のPublish時にまた1つ上げる」を繰り返す。

## Codez バージョニング（CLI表示）

- Codez の CLI 表示バージョンは、`openai/codex` の **GitHub Releases の最新 stable（pre-release除外）** の `rust-vX.Y.Z` を基準にし、`X.Y.Z-codez.N` とする（例: `0.77.0-codez.0`）。
  - 理由: `upstream/main` はリリースタグの系譜と一致しないことがあり、`git describe upstream/main` を基準にすると npm / ネイティブ配布の latest とズレうるため。
- 最新 stable の確認:
  - `git fetch upstream --tags`
  - `python3 scripts/map_upstream_releases.py --repo openai/codex --fetch-tags --remote upstream --tag-prefix rust-v --semver-only --format tsv --limit 200 | awk -F'\t' 'NR==1||$4==0{print}' | head`
    - 先頭の `rust-vX.Y.Z` が最新 stable。
- 更新対象:
  - `codex-rs/Cargo.toml` の `[workspace.package].version`
  - TUIの表示を含む snapshots（`codex-rs/tui*/src/status/snapshots/*.snap` など）
  - `codex-rs/Cargo.lock`
  - `README_codez.md` の例
- Rust 変更後は `cd codex-rs && just fmt` を実行する。

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
- When possible, make `match` statements exhaustive and avoid wildcard arms.
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- When making a change that adds or changes an API, ensure that the documentation in the `docs/` folder is up to date if applicable.
- If you change `ConfigToml` or nested config types, run `just write-config-schema` to update `codex-rs/core/config.schema.json`.

Run `just fmt` (in `codex-rs` directory) automatically after you have finished making Rust code changes; do not ask for approval to run it. Additionally, run the tests:

1. Run the test for the specific project that was changed. For example, if changes were made in `codex-rs/tui`, run `cargo test -p codex-tui`.
2. Once those pass, if any changes were made in common, core, or protocol, run the complete test suite with `cargo test --all-features`. project-specific or individual tests can be run without asking the user, but do ask the user before running the complete test suite.

Before finalizing a large change to `codex-rs`, run `just fix -p <project>` (in `codex-rs` directory) to fix any linter issues in the code. Prefer scoping with `-p` to avoid slow workspace‑wide Clippy builds; only run `just fix` without `-p` if you changed shared crates.

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
- Prefer deep equals comparisons whenever possible. Perform `assert_eq!()` on entire objects, rather than individual fields.
- Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.

### Spawning workspace binaries in tests (Cargo vs Bazel)

- Prefer `codex_utils_cargo_bin::cargo_bin("...")` over `assert_cmd::Command::cargo_bin(...)` or `escargot` when tests need to spawn first-party binaries.
  - Under Bazel, binaries and resources may live under runfiles; use `codex_utils_cargo_bin::cargo_bin` to resolve absolute paths that remain stable after `chdir`.
- When locating fixture files or test resources under Bazel, avoid `env!("CARGO_MANIFEST_DIR")`. Prefer `codex_utils_cargo_bin::find_resource!` so paths resolve correctly under both Cargo and Bazel runfiles.

### Integration tests (core)

- Prefer the utilities in `core_test_support::responses` when writing end-to-end Codex tests.

- All `mount_sse*` helpers return a `ResponseMock`; hold onto it so you can assert against outbound `/responses` POST bodies.
- Use `ResponseMock::single_request()` when a test should only issue one POST, or `ResponseMock::requests()` to inspect every captured `ResponsesRequest`.
- `ResponsesRequest` exposes helpers (`body_json`, `input`, `function_call_output`, `custom_tool_call_output`, `call_output`, `header`, `path`, `query_param`) so assertions can target structured payloads instead of manual JSON digging.
- Build SSE payloads with the provided `ev_*` constructors and the `sse(...)`.
- Prefer `wait_for_event` over `wait_for_event_with_timeout`.
- Prefer `mount_sse_once` over `mount_sse_once_match` or `mount_sse_sequence`

- Typical pattern:

  ```rust
  let mock = responses::mount_sse_once(&server, responses::sse(vec![
      responses::ev_response_created("resp-1"),
      responses::ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
      responses::ev_completed("resp-1"),
  ])).await;

  codex.submit(Op::UserTurn { ... }).await?;

  // Assert request body if needed.
  let request = mock.single_request();
  // assert using request.function_call_output(call_id) or request.json_body() or other helpers.
  ```

# Headings

- `codez` の説明、および `codez` 向けの機能・運用ルールは `README_codez.md` に追記していくこと（一般向けドキュメントや共通仕様に混ぜない）。
- upstream（openai/codex）をマージしたら、`README_codez.md` の「Upstream マージ履歴」を更新して、取り込み対象（tag/branch）とマージコミットを残すこと。
- VSCode拡張の挙動変更をしたら、`vscode-extension/CHANGELOG.md` の `Unreleased` に追記すること（ローカル開発では `vscode-extension/package.json` の `version` は上げない）。

# Git Subtree (vscode-extension)

- `vscode-extension/` は `codez` に subtree で取り込み済みの前提で運用する。
- `codez` と `vscode-extension` の変更は同一コミットに混在して構わない（subtree push 時に `vscode-extension/` 配下のみが抽出される）。
- 元リポジトリと同期したい場合は手動で `git subtree pull/push` を使う（submodule のような自動リンクではない）。

# Marketplace Publish（vscode-extension）

- Marketplace への `vsce publish` は **ユーザーが明示的に「Publishして」と指示した時のみ**実行する。
- 指示がない場合は、ローカルの `vsix:package` / `code --install-extension` までに留める。
- `vsce unpublish` は拡張を **Marketplace から完全に削除（全バージョン削除）**する挙動で、以前のバージョンへ戻す操作ではないため、**今後は実行しない**（必要なら Web 管理画面での手順含めてユーザーに確認する）。
