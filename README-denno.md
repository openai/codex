# Codex-DENNO README

このリポジトリは OpenAI の Codex CLI（openai/codex）のフォークで、エージェントが扱える出力サイズを拡張した「`codex-denno`」バイナリをチーム内で使うためのものです。

## 何が違うのか

- モデルに渡すときのトランケーション閾値を拡張しています。
  - バイト数上限: **約 10KiB → 100KiB**
  - 行数上限: **256 行 → 2000 行**
- 画面に出る CLI の生の出力は従来どおりフルですが、モデルに返すサマリ文字列が大きく取れるようになっています。
- 公式版 `codex` はそのまま残しつつ、拡張版を `codex-denno` という別コマンドとしてインストールします。

## インストール（macOS / Linux / WSL）

前提:

- Rust / cargo がインストールされていること
- このレポジトリを clone 済みであること

手順:

```bash
cd /path/to/codex-denno
./scripts/install-codex-denno.sh           # デフォルト: ~/.local/bin/codex-denno にインストール
```

PATH に `~/.local/bin` を追加していない場合は、`~/.zshrc` などに次を追加してください:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

その後、新しいシェルで:

```bash
codex-denno --help
```

と打ってヘルプが表示されればインストール完了です。

### WSL での利用

- **WSL は Linux と同じ扱い**なので、上記とまったく同じ手順で動きます。
  - 例: `\\wsl$\Ubuntu\home\USER\codex-denno` に clone → `./scripts/install-codex-denno.sh`
- VS Code の「WSL: Ubuntu」ウィンドウからターミナルを開いて実行すれば、そのまま `codex-denno` が使えます。
- Windows 側から WSL 内の Codex を直接叩きたい場合は、例えば:
  - `wsl codex-denno ...` のように `wsl` 経由で呼び出す
  - もしくは WSL 側だけで Codex を使う（推奨）

## 公式版との併用

- 公式版 CLI:
  - 例: `codex`（npm でインストールしたもの）が従来の 10KiB / 256 行。
- 拡張版 CLI:
  - `codex-denno` が 100KiB / 2000 行。

用途に応じてコマンド名を切り替えるだけで共存できます。

## TypeScript SDK から `codex-denno` を使う

TypeScript SDK（`sdk/typescript`）は内部で `codex` バイナリを `spawn` していますが、オプションでパスを上書きできます。

プロジェクト側のコード例:

```ts
import { Codex } from "@openai/codex-sdk";

const codex = new Codex({
  // PATH 上の codex-denno を使う
  codexPathOverride: "codex-denno",
});
```

チーム内では、以下を共通ルールにすると分かりやすいです。

- Codex を SDK 経由で呼ぶときは必ず `codexPathOverride: "codex-denno"` を指定する。
- 公式版を使いたいツールがあれば、そちらでは `codexPathOverride` を指定しない（デフォルトの vendor バイナリを使う）。

## Windows ネイティブで使いたい場合（参考）

本来のターゲットは macOS / Linux / WSL ですが、Windows ネイティブで試したい場合のメモです。

1. Rust (MSVC toolchain) をインストールする。
2. PowerShell などで:

   ```powershell
   cd path\to\codex-denno\codex-rs
   cargo build -p codex-cli --release --target x86_64-pc-windows-msvc
   ```

3. 出来上がった `target\x86_64-pc-windows-msvc\release\codex.exe` を任意のディレクトリにコピーし、`codex-denno.exe` として PATH に通す。

ただし、実運用では **WSL 上で `codex-denno` を使う構成を推奨**します（挙動が Linux/macOS と揃うため）。***

