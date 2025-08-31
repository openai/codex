# Slide CLI 要件定義（MVP）

## 概要 / 目的
- 端末（ターミナル）で実行する最小構成のエージェント型 CLI「Slide」を提供する。
- `slide` コマンドで起動し、会話（チャット）から Markdown スライド（.md）を生成する。
- 初回挙動は既存 Codex CLI に近い UX とし、安全なサンドボックス実行・承認フローを踏襲する（同等で十分）。
- 生成された Markdown をユーザーが任意ツールで PPTX 化することを前提（本MVPでは変換機能は非対象）。

## スコープ（MVP）
- `slide` コマンドの提供（Node 薄いラッパ or 既存 Rust バイナリ起動の別名）。
- 対話（チャット）からスライド要件を受け取り、Markdown ファイルを生成。
- 出力先はリポジトリ直下の `slides/` ディレクトリ（無ければ作成）。
- 承認ポリシー（suggest/auto-edit/full-auto）およびサンドボックス方針は Codex と同等。
- 既存のモデル・プロバイダ設定は流用（モデル選定はデフォルト `o4-mini` 想定）。
- プレビューTUI（最小版）の提供（後述）。

## 非スコープ（MVP）
- PPTX/PDF への自動変換（将来対応）。
- 画像生成・テーマ選択 UI。
- 外部 MCP 連携（MVPでは必須ではない）。

## プレビューTUI（MVP）
- 目的: 生成済み Markdown をターミナルで素早く確認・ページ送りする。
- 技術: Rust ratatui（既存 `codex-rs/tui` のスタイル方針に準拠）。
- 起動方法:
  - `slide preview <path/to/markdown>`
  - または生成直後に「プレビューを開く？」で `y` 選択
- 表示仕様:
  - 左/右でスライド移動（`j/k` もしくは `←/→`）
  - 現在ページ/総ページのステータス表示
  - ANSI スタイルは `ansi-escape` を利用して最低限の装飾レンダ
- 非対応（MVP）:
  - 画像のインライン描画、テーマ切替、ライブ編集反映
- 終了キー: `q`

## ユースケース例
- 「営業向け 10 枚の提案資料を作って」→ 章立て・要点・箇条書きで Markdown 生成。
- 「この仕様書から要約スライド 5 枚」→ 指定ファイルを読み要約して Markdown 生成。
- 「日本語/英語で作って」→ 言語指定で出力。

## CLI 仕様（MVP）
- コマンド名: `slide`
- 動作モード:
  - `slide`（インタラクティブ REPL）
  - `slide "<プロンプト>"`（ワンショット開始）
- 主なフラグ（Codex と共通に揃える）
  - `-a, --approval-mode <suggest|auto-edit|full-auto>`
  - `-m, --model <name>`
  - `-q, --quiet`、`--json`（必要に応じて）
- ファイル生成アクションは承認プロンプトを経由（auto-edit/full-auto では自動）。

## 生成物仕様（Markdown）
- 出力先: `slides/<timestamp>_<slug>.md`
- 推奨フォーマット:
  - 1行目: タイトル（`# タイトル`）
  - スライドは `##` 区切り（1スライド=1セクション）
  - 箇条書き・コード・画像プレースホルダを許容（画像はURL/相対パス）
- 例:
  ```md
  # プロジェクト提案 2025Q1

  ## 1. 課題
  - 現状の問題点
  - 影響範囲

  ## 2. 解決方針
  - 施策A
  - 施策B
  ```

## 初回起動時の挙動
- Codex 同等のオンボーディング（モデル・承認モード・通知など）を簡易にガイド。
- 必要最小限の設定ファイル（`~/.slide/config.(json|yaml)` or 既存 `~/.codex/config.*` を流用）
  - MVPでは Codex の設定ファイルを流用し、`provider/model/approvalMode` を尊重。

## セキュリティ / 実行方針
- Codex と同様にネットワーク無効（Full Auto時も既定は無効）・作業ディレクトリ内に限定。
- Linux ではコンテナ/iptables によるサンドボックスを利用可能（将来互換）。
- ファイル書き込みは承認制（モード依存）。

## ログ / トレース
- `RUST_LOG` / `DEBUG` など既存と同等の出力を踏襲。
- 生成結果（パス）を確実にユーザーへ表示。

## 実装方針（MVP 最短）
1) CLI ラッパ（Node）
   - `slide-cli/` を `codex-cli/` と同構成で作成し、`bin/slide.js` を追加。
   - プラットフォーム判定 → `bin/slide-<target>` を起動（codex と同形式）。
   - 互換最優先のため、Rust 側に `--app slide` などのモードを伝える（環境変数でも可）。

2) Rust 側（codex-rs/cli）
   - `argv0` もしくは `--app slide` で「Slideモード」を有効化。
   - デフォルトのカスタム指示（スライド出力を志向）を適用。
   - ファイル生成アクションのユーティリティ（`slides/` 作成・ファイル命名・上書き確認）。

3) 最小チャット体験
   - REPLで「スライドテーマ/枚数/対象読者/口調/画像有無」等を引き出す簡易プロンプト。
   - 1回目の下書きMDを生成 → 承認 → `slides/` に保存。
   - 再生成/追記/分割などは将来拡張。

## 受け入れ基準（MVP）
- `slide "営業向け提案 10枚 日本語で"` で `slides/*.md` が生成される。
- インタラクティブで追記指示を出すと、上記ファイルに安全に追記（または別名保存）できる。
- suggest/auto-edit/full-auto の各モードで意図どおりに承認フローが機能。

## テスト方針（最小）
- 単体: ファイル命名・スラッグ生成・出力ディレクトリ作成のテスト。
- 統合: 固定プロンプトで Markdown を生成し、想定のヘッダ/セクション数を満たすか検証。

## 今後の拡張
- 画像生成（ダイアグラム/サムネイル）・テーマ（色/フォント）・テンプレート対応。
- PPTX/PDF 自動変換（Pandoc/Marp 等連携の検討）。
- 外部 MCP の利用（社内ナレッジ検索、図表自動作成ツールなど）。

## 未決事項 / 要確認
- パッケージ名（npm）: 仮 `@yourorg/slide`（要確定）。
- 設定ファイル: `~/.slide/*` 新設か `~/.codex/*` 流用か（MVPは流用案）。
- リリース戦略: codex と同一トリプル/同梱方式で問題ないか。

## 想定ディレクトリ構造（MVP）
```
repo-root/
├─ codex-cli/                # 既存: Codex 用 Node ランチャ
│  └─ ...
├─ codex-rs/                 # 既存: Rust ワークスペース
│  ├─ cli/                   # 既存: Rust CLI 本体
│  ├─ core/                  # 既存: コア機能
│  ├─ ...
│  └─ (他の既存クレート)
├─ slide-cli/                # 新規: Slide 用 Node ランチャ（codex-cli と同構成）
│  ├─ package.json
│  ├─ bin/
│  │  └─ slide.js           # エントリポイント（プラットフォーム判定→ slide-<target> 実行）
│  ├─ scripts/               # （必要なら）リリース・サンドボックス補助
│  └─ (bin/slide-<target> はリリース時に同梱)
├─ docs/
│  ├─ slide-cli-requirements.md
│  └─ ...
├─ slides/                   # 生成物（Markdown）出力先（初回生成時に作成）
│  └─ <timestamp>_<slug>.md
└─ ...
```

- Node ランチャを分ける理由: コマンド名 `slide` を npm 経由で配布しやすくするため（`codex` と並存）。
- Rust 側は既存 CLI にモード追加（`--app slide` or argv0）で最小改修方針。
- 将来、共通化できる箇所（スクリプト・配布手順）は `codex-cli` と揃える。

## 実装しない機能（MVP明確化）
- PPTX/PDF への自動変換（手動で別ツールに渡す前提）
- テーマ選択・カスタム配色・フォント変更
- スライドテンプレート（高度なテンプレ）
- 画像の自動生成・埋め込み（URLプレースホルダは許容）
- 外部 MCP 連携（社内検索・SaaS連携など）
- リアルタイム共同編集（ライブコラボ）
- 自動要約の高精度評価/スコアリング（簡易生成のみ）
- TUIでのライブ編集反映（MVPは固定表示 + ページング）

## 動作フロー（エンドツーエンド）
1) インストール
   - npm 経由で `slide` を導入（`@yourorg/slide` 想定）。
   - 初回起動でモデル/APIキーなどを案内（Codex同等の簡易ガイド）。

2) 起動モード
   - 対話開始: `slide`
   - ワンショット: `slide "<要件を1行で>"`

3) 対話（チャット）
   - モデルへ質問: テーマ/枚数/対象/口調/画像有無/言語など
   - 合意内容からスライド素案を生成（Markdownテキスト）

4) 承認と保存
   - ファイル内容を表示→承認
   - `slides/<timestamp>_<slug>.md` を作成（既存と衝突する場合は別名）

5) プレビュー（任意）
   - `slide preview slides/<file>.md` でTUI起動
   - ←/→（j/k）でページ遷移、`q` で終了

6) 後続（ユーザー側）
   - 生成Markdownを任意ツールで PPTX/PDF へ変換
   - 必要なら再度 `slide` で修正案を相談→追記/再生成

7) 失敗時の扱い
   - 書き込み失敗や生成不十分な場合は差分提示・再試行をガイド
   - Quiet/JSON モードでは機械可読な結果を返す

## 実装コード（計画反映）

### slide-cli/package.json
```json
{
  "name": "@yourorg/slide",
  "version": "0.0.1",
  "license": "Apache-2.0",
  "bin": {
    "slide": "bin/slide.js"
  },
  "type": "module",
  "engines": {
    "node": ">=20"
  },
  "files": [
    "bin",
    "dist"
  ],
  "dependencies": {
    "@vscode/ripgrep": "^1.15.14"
  },
  "devDependencies": {
    "prettier": "^3.3.3"
  }
}
```

### slide-cli/bin/slide.js
```javascript
#!/usr/bin/env node
// Slide CLI launcher: selects platform-specific native binary and runs it.

import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

let targetTriple = null;
switch (platform) {
  case "linux":
  case "android":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-unknown-linux-musl";
        break;
      case "arm64":
        targetTriple = "aarch64-unknown-linux-musl";
        break;
      default:
        break;
    }
    break;
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-apple-darwin";
        break;
      case "arm64":
        targetTriple = "aarch64-apple-darwin";
        break;
      default:
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-pc-windows-msvc.exe";
        break;
      default:
        break;
    }
    break;
  default:
    break;
}

if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const binaryPath = path.join(__dirname, "..", "bin", `slide-${targetTriple}`);

const { spawn } = await import("child_process");

async function tryImport(moduleName) {
  try {
    return await import(moduleName);
  } catch {
    return null;
  }
}

async function resolveRgDir() {
  const ripgrep = await tryImport("@vscode/ripgrep");
  if (!ripgrep?.rgPath) return null;
  return path.dirname(ripgrep.rgPath);
}

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  return [...newDirs, ...existingPath.split(pathSep).filter(Boolean)].join(pathSep);
}

const additionalDirs = [];
const rgDir = await resolveRgDir();
if (rgDir) additionalDirs.push(rgDir);
const updatedPath = getUpdatedPath(additionalDirs);

// Pass a hint so the native binary can switch to Slide mode if shared
const env = { ...process.env, PATH: updatedPath, SLIDE_APP: "1" };

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

const forwardSignal = (signal) => {
  if (child.killed) return;
  try { child.kill(signal); } catch {}
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) resolve({ type: "signal", signal });
    else resolve({ type: "code", exitCode: code ?? 1 });
  });
});

if (childResult.type === "signal") {
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
```

### slide-cli/README.md
```markdown
# Slide CLI (MVP)

Lightweight terminal agent to generate Markdown slides via chat. Ships as a Node launcher that executes platform-specific native binaries.

## Quickstart
```
npm i -g @yourorg/slide
slide
slide "営業向け提案 10枚 日本語で"
```

## Preview TUI (MVP)
```
slide preview slides/<file>.md
```

- Navigate: ←/→ (or j/k)
- Quit: q

## Notes
- Generates Markdown into `slides/` as `<timestamp>_<slug>.md`.
- PPTX/PDF conversion is out of scope for MVP (use external tools).
```

### slide-cli/scripts/README.md
```markdown
# scripts (optional for MVP)

This directory is reserved for release and sandbox helpers (e.g., staging native binaries).
For the MVP, no scripts are required. Keep this as a placeholder for future tooling.
```

### slides/sample.md
```markdown
# デモスライド：Slide CLI MVP

## 1. 目的
- ターミナルからチャットでスライドを生成
- Markdown を生成し、ユーザーが任意ツールで変換

## 2. スコープ（MVP）
- `slide` コマンド
- チャット→Markdown 生成
- 簡易プレビューTUI（ページ送り）

## 3. 次の一歩
- テンプレート/テーマ対応
- 画像サポート
- PPTX/PDF 変換連携
```
