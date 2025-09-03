# 最小構成ガイド: コマンド一発で起動する AI エージェント CLI

このドキュメントは「npm で入れて、`myagent` コマンドで起動 → 対話入力 → エージェントが応答」という最小構成を一から作るためのガイドです。

このリポジトリ（Codex）は次の構成が特徴です:
- Node.js の CLI ラッパ（ESM）から、プラットフォーム別のネイティブバイナリ（Rust 製）を起動する

よって本ガイドでは、次の2パターンを示します。
- パターンA: まず動く最小の Node.js 単体版（最短で試す）
- パターンB: このリポジトリに近い構成（Node ラッパ + ネイティブバイナリ）

---

## TL;DR / 結論

はい。パターンAの内容だけで「コマンドを叩く → 起動 → チャット入力に応答」まで動作します（LLM なしの簡易応答）。

LLM による会話も、`src/llm.js` を足して `respond()` を差し替え、API キー（例: `OPENAI_API_KEY`）を設定すれば、そのまま同じコマンドで動かせます。追加のビルドや特殊設定は不要です。

以下に「すぐ試せる検証手順」と「つまずきやすいポイント」を補足します。

---

## パターンA: Node.js 単体（最小）

### ディレクトリ構成

```
myagent/
  package.json
  bin/
    myagent.js        # エントリーポイント（CLI コマンド本体）
  src/
    agent.js          # 対話ループと簡易エージェント
  README.md           # 任意（使い方）
  .gitignore          # 任意
```

- 必須なのは `package.json`, `bin/myagent.js`, `src/agent.js` の3点です。
- 将来的に LLM 接続や設定を分割したくなったら `src/llm.js`, `src/config.js` などを足せば OK です。

---

## 各ファイルの最小内容

### 1) `package.json`

- 重要なのは `bin` フィールドで、コマンド名 → 実行ファイルへのパスを宣言することです。
- ここでは CommonJS（`require`）で最小構成にしています。

```json
{
  "name": "myagent",
  "version": "0.1.0",
  "private": false,
  "license": "MIT",
  "type": "module",
  "engines": { "node": ">=20" },
  "bin": {
    "myagent": "./bin/myagent.js"
  },
  "scripts": {
    "start": "node bin/myagent.js",
    "format": "prettier -w ."
  },
  "dependencies": {},
  "devDependencies": {
    "prettier": "^3.3.3"
  }
}
```

ポイント:
- `bin` によって、グローバルインストール（または `npm link`）後に `myagent` で起動できます。
- 依存はゼロ。標準モジュールのみで動かします（最小）。

---

### 2) `bin/myagent.js`（実行ファイル）

- shebang を付け、`src/agent.js` の `run()` を呼ぶだけの薄いラッパです。
- このファイルに実行権限を付けるのを忘れずに（`chmod +x bin/myagent.js`）。

```js
#!/usr/bin/env node
import { run } from '../src/agent.js';

try {
  await run();
} catch (err) {
  console.error(err);
  process.exit(1);
}
```

---

### 3) `src/agent.js`（最小の対話ループ）

- Node.js の `readline` だけでシンプルなチャットを実装します。
- `exit` / `quit` で終了。未接続でも「エージェントが動く感じ」を出すため、超簡易な応答を返します。

```js
import readline from 'node:readline';

export async function run() {
  console.log('MyAgent — type "exit" to quit');

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: '> '
  });

  rl.prompt();

  rl.on('line', async (line) => {
    const input = line.trim();

    if (!input) {
      rl.prompt();
      return;
    }

    if (['exit', 'quit', ':q'].includes(input.toLowerCase())) {
      rl.close();
      return;
    }

    const reply = await respond(input);
    console.log(reply);
    rl.prompt();
  });

  rl.on('close', () => {
    console.log('Bye!');
    process.exit(0);
  });
}

async function respond(userInput) {
  // 最小ダミー応答（ここを LLM 連携に差し替える）
  if (/hello|hi|hey|こんにちは|こんちは/i.test(userInput)) {
    return 'こんにちは！どんなことを手伝えますか？';
  }
  if (/help|使い方|help\?/i.test(userInput)) {
    return 'キーワードを入力してください。"exit" で終了します。';
  }
  return `You said: ${userInput}`;
}
```

---

## 動かし方（ローカル開発）

```bash
# 雛形を作成
mkdir myagent && cd myagent
npm init -y

# 上記ファイル/フォルダを作成して保存
# 実行権限を付与（macOS/Linux）
chmod +x bin/myagent.js

# 実行（ローカル）
node bin/myagent.js

# または、開発中にコマンドで叩きたい場合
npm link   # これで `myagent` がグローバルにリンクされる
myagent
```

- 公開後は `npm i -g myagent` でインストール、または `npx myagent` で即時実行できます。

### 動作確認クイックスタート（期待される入出力）

```text
$ node bin/myagent.js
MyAgent — type "exit" to quit
> hello
こんにちは！どんなことを手伝えますか？
> what is this?
You said: what is this?
> exit
Bye!
```

こうなれば成功です。`myagent` 経由でも同じ入出力になります。

### つまずきやすいポイント（Node 版）

- Node バージョン: `"engines": { "node": ">=20" }` を満たすこと。
- 実行権限: `bin/myagent.js` に `chmod +x` を忘れない。
- ESM/CJS の整合性: `type: "module"` の場合は `import` を使う。CJS にするなら `type` を外して `require` 化する。
- パス解決: `npm link` を使わない場合は `node bin/myagent.js` で直接実行する。
- Windows の shebang: `node bin/myagent.js` での実行は問題なし。`myagent` コマンド実行時にうまく動かない場合は `CRLF` 改行を `LF` に変更し直す。

---

## LLM 連携を足す（任意・最小例）

最小構成を壊さない範囲で、`src/llm.js` を追加し `respond()` で呼び出す形にします。以下は OpenAI SDK の例（要: `npm i openai` と `OPENAI_API_KEY`）。

`src/llm.js`:

```js
// 依存: npm i openai
import OpenAI from 'openai';

const client = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });

export async function chat(messages) {
  // messages: [{ role: 'user'|'assistant'|'system', content: '...' }, ...]
  const res = await client.chat.completions.create({
    model: 'gpt-4o-mini',
    messages
  });
  return res.choices?.[0]?.message?.content ?? '';
}
```

`src/agent.js` 側の差し替え（抜粋）:

```js
// ファイル先頭付近
import { chat } from './llm.js';

// respond を置き換え
async function respond(userInput) {
  return await chat([
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: userInput }
  ]);
}
```

注意:
- ネットワークに依存するため、テストや CI ではモック化したり、機能フラグで無効化するのがおすすめです。
- モデル名や API 呼び出しはお好みで調整してください。

### LLM 連携の通し手順（例: OpenAI）

```bash
# 依存追加
npm i openai

# API キーをシェルに設定
export OPENAI_API_KEY=...   # Windows PowerShell: $env:OPENAI_API_KEY='...'

# 実行
node bin/myagent.js
```

プロキシ/企業ネットワーク環境では、HTTPS プロキシ設定が必要な場合があります（例: `HTTPS_PROXY`）。

Codex リポジトリ（本体）では既定でネットワークが制限されるモードがありますが、本最小サンプルは素の Node 実行のため制限はありません。

---

## よくある追加（必要になったら）

- 設定管理: `src/config.js`（環境変数/設定ファイルの読み込み）
- 履歴保存: `~/.myagent/history.jsonl` に書き出す（ネットワークなしでも快適に）
- TUI 化: 必要に応じて `ink` や `blessed` を使って UI を強化
- ロギング: `debug` パッケージや簡単な `console.debug` ラッパ
- 配布: `npm publish` で公開（`name` はユニークに）

### スコープ配布: `@taiyo/slide` として公開・起動する

スコープ付きパッケージ（例: `@taiyo/slide`）として公開し、`npm install -g @taiyo/slide` の後に `slide` コマンドで起動できるようにする最小設定例です。

1) `package.json`（例: Node 単体のパターンA）

```json
{
  "name": "@taiyo/slide",
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "engines": { "node": ">=20" },
  "bin": {
    "slide": "./bin/slide.js"
  },
  "files": ["bin", "src"],
  "scripts": {
    "start": "node bin/slide.js"
  },
  "dependencies": {},
  "devDependencies": {
    "prettier": "^3.3.3"
  }
}
```

ポイント:
- コマンド名は `bin` のキー（ここでは `slide`）。インストール後に `slide` で起動します。
- スコープ付きパッケージを公開する場合は初回 `npm publish --access public` が必要です。

2) エントリ `bin/slide.js`

```js
#!/usr/bin/env node
import { run } from '../src/agent.js';

try {
  await run();
} catch (err) {
  console.error(err);
  process.exit(1);
}
```

3) 起動確認（グローバルインストール）

```bash
npm install -g @taiyo/slide
slide               # グローバルコマンドで起動
```

4) 即時実行（グローバル不要の一発実行）

```bash
npx @taiyo/slide
```

パターンB（Node ラッパ + ネイティブ）で同様に配布する場合は、`bin/slide.js` をラッパ、`bin/slide-<triple>` に各プラットフォームのネイティブ実行ファイルを同梱し、`package.json` の `files` で `bin/` を含める構成にします。Codex の `codex-cli/bin/codex.js` が実例です（このリポジトリ内）。

### 履歴の保存（最小 JSONL 例）

`~/.myagent/history.jsonl` に 1 行 1 メッセージで追記する簡易実装例です。

```js
// src/history.js
import { mkdir, appendFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

const dir = join(homedir(), '.myagent');
const file = join(dir, 'history.jsonl');

export async function saveMessage(role, content) {
  await mkdir(dir, { recursive: true });
  const line = JSON.stringify({ ts: Date.now(), role, content }) + '\n';
  await appendFile(file, line, 'utf8');
}
```

`src/agent.js` の入出力で `saveMessage('user', input)`, `saveMessage('assistant', reply)` を呼び出すだけで履歴がたまります。

### 簡易テスト（スモーク）

```bash
node bin/myagent.js << 'EOF'
hello
exit
EOF
```

非ゼロ終了コードや例外が出ないことを確認できます。CI ではこのスモークに追加して `node -e` でモジュールの import 成功も検証すると堅牢です。

---

## パターンB: Node ラッパ + ネイティブバイナリ（Codex に近い構成）

Codex は Node からプラットフォーム別のネイティブ実行ファイルを起動します。配布サイズや実行速度、TUI/OS 機能の活用に有利です。

### ディレクトリ構成（例）

```
myagent/
  package.json              # ESM + Node>=20
  bin/
    myagent.js              # ラッパ（ESM）
    myagent-x86_64-apple-darwin
    myagent-aarch64-apple-darwin
    myagent-x86_64-unknown-linux-musl
    myagent-aarch64-unknown-linux-musl
    myagent-x86_64-pc-windows-msvc.exe
  native/                   # 任意: ソース管理する場合
    Cargo.toml
    src/main.rs
```

最小 `package.json`（ESM）:

```json
{
  "name": "myagent",
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "engines": { "node": ">=20" },
  "bin": { "myagent": "./bin/myagent.js" },
  "dependencies": {}
}
```

`bin/myagent.js`（プラットフォーム検出と起動）:

```js
#!/usr/bin/env node
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

/**
 * 簡略版: 主要4プラットフォームのみ。必要に応じて拡張してください。
 */
function detectTriple() {
  if (platform === 'darwin') {
    if (arch === 'arm64') return 'aarch64-apple-darwin';
    if (arch === 'x64') return 'x86_64-apple-darwin';
  }
  if (platform === 'linux') {
    if (arch === 'arm64') return 'aarch64-unknown-linux-musl';
    if (arch === 'x64') return 'x86_64-unknown-linux-musl';
  }
  if (platform === 'win32' && arch === 'x64') {
    return 'x86_64-pc-windows-msvc.exe';
  }
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const triple = detectTriple();
const binPath = path.join(__dirname, `myagent-${triple}`);

const child = spawn(binPath, process.argv.slice(2), { stdio: 'inherit' });
child.on('error', (err) => {
  console.error(err);
  process.exit(1);
});
child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
```

オプション: 追加バイナリの PATH 注入（`ripgrep` 等）

Codex は `@vscode/ripgrep` を依存に持ち、その実体パスを `PATH` に注入してから子プロセスを起動しています。必要に応じて、以下のように書き換えます。

```js
// 依存: npm i @vscode/ripgrep
import * as ripgrep from '@vscode/ripgrep';

function withExtraPath(env) {
  const sep = process.platform === 'win32' ? ';' : ':';
  const extra = ripgrep?.rgPath ? [require('node:path').dirname(ripgrep.rgPath)] : [];
  const existing = (env.PATH || '').split(sep).filter(Boolean);
  return { ...env, PATH: [...extra, ...existing].join(sep) };
}

const child = spawn(binPath, process.argv.slice(2), {
  stdio: 'inherit',
  env: withExtraPath(process.env),
});
```

Rust 側（最小例）: `native/src/main.rs`

```rust
use std::io::{self, Write};

fn main() {
    println!("MyAgent (native) — type 'exit' to quit");
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        print!("> ");
        stdout.flush().ok();
        let mut buf = String::new();
        if stdin.read_line(&mut buf).is_err() { break; }
        let input = buf.trim();
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") { break; }
        println!("You said: {}", input);
    }
}
```

ビルドと配置（例）:

```bash
cd native
cargo build --release                 # まずはホスト向けのみ
# 出力を myagent/bin へ配置し、上記の命名にリネーム
cp target/release/myagent ../bin/myagent-aarch64-apple-darwin   # 例: macOS/arm64
chmod +x ../bin/myagent-aarch64-apple-darwin
```

クロスコンパイルは環境構築が必要になるため、まずは開発用にホスト OS 向けだけ置き、後で必要なプラットフォームを増やすのがおすすめです。

注意:
- Windows は拡張子 `.exe` が必要です。
- 配布時は不要なファイルを `files` フィールドで制限するとよいです（`bin/` のみなど）。

推奨 `package.json` の追加フィールド例:

```json
{
  "files": ["bin"],
  "engines": { "node": ">=20" }
}
```

---

## Codex リポジトリ実装との対応関係（このリポジトリの具体例）

本リポジトリは、上記パターンBの構造をそのまま拡張したものです。主要対応は次のとおりです。

- codex-cli（Node ラッパ）:
  - 実体: `codex-cli/bin/codex.js`
  - 役割: プラットフォーム/アーキテクチャからターゲットトリプルを判定し、同梱のバイナリ `bin/codex-<triple>` を `spawn` で起動。
  - 追加: `@vscode/ripgrep` を依存に持ち、そのパスを `PATH` に注入してから子プロセスを実行。
  - 参考: `codex-cli/package.json` の `bin`, `type: "module"`, `engines: { node: ">=20" }`

- codex-rs（Rust ワークスペース）:
  - CLI バイナリ: `codex-rs/cli` クレートが `[[bin]] name = "codex"` を出力。
  - TUI コア: `codex-rs/tui` クレート（名前は `codex-tui`）が TUI 機能を提供。
  - ビルド例: `cd codex-rs && cargo build -p codex-cli --release` で `codex` バイナリを生成。
  - 配布時: npm パッケージ内ではプラットフォーム別に `codex-<triple>` として同梱（Node ラッパから起動）。

- ドキュメント/使い方:
  - 実行方法の概要: `docs/getting-started.md`（`codex`, `codex exec` などのサブコマンド）
  - サンドボックスと承認ポリシー: `docs/sandbox.md`, `docs/platform-sandboxing.md`

この最小構成ガイドの Node ラッパ例と `codex-cli/bin/codex.js` はほぼ一致しており、PATH 注入やシグナル転送などの実装は Codex の実装を簡略化して示しています。

---

## オプション: AGENTS.md による「メモリ」

Codex は `AGENTS.md` を読み取り、ガイダンスをプロンプトに合成します。最小実装でも同様にするなら、次の場所を順に探すのが分かりやすいです（どれか存在すれば結合）。

1. `~/.myagent/AGENTS.md`（グローバル）
2. カレントワークスペース直下の `AGENTS.md`

Node 版（簡易実装）:

```js
import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

async function readAgentsMd() {
  const candidates = [
    join(homedir(), '.myagent', 'AGENTS.md'),
    join(process.cwd(), 'AGENTS.md'),
  ];
  const texts = [];
  for (const p of candidates) {
    try { texts.push(await readFile(p, 'utf8')); } catch {}
  }
  return texts.join('\n\n');
}

// LLM 呼び出し時に system メッセージで合成する例
// const system = await readAgentsMd();
```

注意: ファイル探索コストが気になる場合、起動時に一度だけ読み取ってキャッシュするか、明示コマンドで再読込する設計が扱いやすいです。

---

## セキュリティ/サンドボックスの考え方（指針）

Codex は OS サンドボックス（macOS: Seatbelt、Linux: Landlock+seccomp）を使い、既定でネットワークを遮断します。最小実装ではここまで不要でも、次の方針を取り入れると安全に運用できます。

- 扱う権限をモード化:
  - `read-only`: 読み取りのみ
  - `workspace-write`: ワークスペース配下の書込可（`.git/` は保護）
  - `danger-full-access`: 制限なし（検証環境のみ）
- 承認フロー:
  - `on-request`: モデルが必要時のみ昇格要求
  - `on-failure`: 失敗時に昇格確認
  - `never`: いかなる昇格もしない

ネットワーク依存をテストに持ち込まない・機能フラグで無効化する、という本リポジトリの方針も推奨です。

---

## まとめ

- 最小で必要なのは、`package.json` + `bin/` + `src/`（パターンA）。
- このリポジトリに近づけるなら、`bin/myagent.js`（ESM）でプラットフォーム別バイナリを起動（パターンB）。
- LLM 連携は後から `src/llm.js` を足して差し替えるだけで OK。
 - 必要に応じて PATH 注入・`AGENTS.md` 読み込み・サンドボックス/承認モードを段階的に採用。
