# 🔊 Codex 音声通知機能

Codexのタスク完了時に音声で通知する機能です。長時間実行タスクの完了を音声で知らせることで、作業効率を向上させます。

## 📋 概要

霊夢の「終わったわ」音声をCodexのタスク完了時に自動再生します。

**参考記事**: [akademeia.info - Codex音声通知の実装](https://akademeia.info/?p=43790)

## 🎯 機能

- **タスク完了時**: `on_task_complete`フックで音声再生
- **セッション終了時**: `on_session_end`フックで音声再生
- **非同期実行**: 音声再生は同期的に実行され、完了を確実に通知
- **エラーハンドリング**: 音声ファイルが見つからない場合は警告を表示

## 📦 セットアップ

### 1. 音声ファイルの準備

1. 霊夢の「終わったわ」音声をWAV形式で用意
2. ファイル名を `reimu_owattawa.wav` に変更
3. 以下のディレクトリに配置:

```
codex-main/
└── zapabob/
    └── scripts/
        ├── play-completion-sound.ps1
        └── reimu_owattawa.wav  ← ここに配置
```

### 2. 設定確認

`config.toml`に以下の設定が追加されていることを確認:

```toml
# ==================== フック機能 ====================
# タスク完了時の音声通知
[hooks]
on_task_complete = "pwsh zapabob/scripts/play-completion-sound.ps1"
on_session_end = "pwsh zapabob/scripts/play-completion-sound.ps1"
```

### 3. テスト実行

```powershell
# スクリプト単体でテスト
cd codex-main
pwsh zapabob/scripts/play-completion-sound.ps1

# Codexタスクで実際にテスト
codex exec "echo 'test task'"
```

## 🔧 カスタマイズ

### 音声ファイルの変更

別の音声ファイルを使用する場合:

1. WAV形式の音声ファイルを用意
2. `zapabob/scripts/`ディレクトリに配置
3. `play-completion-sound.ps1`の`$wavPath`を変更:

```powershell
$wavPath = Join-Path $PSScriptRoot "your-sound-file.wav"
```

### フックのカスタマイズ

`config.toml`で異なるイベントにフックを追加:

```toml
[hooks]
on_task_start = "pwsh zapabob/scripts/play-start-sound.ps1"
on_task_complete = "pwsh zapabob/scripts/play-completion-sound.ps1"
on_error = "pwsh zapabob/scripts/play-error-sound.ps1"
on_session_end = "pwsh zapabob/scripts/play-completion-sound.ps1"
```

### 非同期再生に変更

音声再生を非同期（バックグラウンド）で実行する場合:

```powershell
# PlaySync() の代わりに Play() を使用
$player.Play()  # 非同期再生（すぐに制御が戻る）
```

## 🎨 使用例

### 基本的な使用

```powershell
# Codexタスクを実行（完了時に音声再生）
codex exec "レビューして"

# 長時間タスク（完了時に通知されるので便利）
codex delegate code-reviewer --scope ./src
```

### Codex Agent との組み合わせ

```powershell
# 自然言語でタスク実行（完了時に音声通知）
codex agent "セキュリティ重視でコードレビュー"
```

### 並列実行との組み合わせ

```powershell
# 並列タスク完了時に通知
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests
```

## 🐛 トラブルシューティング

### 音声が再生されない

**原因1**: 音声ファイルが見つからない
```powershell
# パスを確認
ls zapabob/scripts/reimu_owattawa.wav
```

**原因2**: WAVフォーマットが正しくない
```powershell
# ファイル情報を確認
Get-Item zapabob/scripts/reimu_owattawa.wav | Format-List
```

**原因3**: フック設定が有効になっていない
```powershell
# config.tomlの設定を確認
codex --config hooks
```

### スクリプトの実行権限エラー

```powershell
# PowerShell実行ポリシーを確認
Get-ExecutionPolicy

# 必要に応じて変更
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### UTF-8エンコーディングエラー

スクリプトはUTF-8でエンコードされています。エラーが発生する場合:

```powershell
# BOM付きUTF-8で保存し直す
Get-Content play-completion-sound.ps1 | Out-File play-completion-sound.ps1 -Encoding UTF8
```

## 📊 パフォーマンス

- **再生時間**: 通常1-2秒（音声ファイルの長さに依存）
- **メモリ使用量**: 約5-10MB（System.Media.SoundPlayer）
- **CPU負荷**: 最小限（再生中のみ）

## 🔒 セキュリティ

- **サンドボックス対応**: 音声再生はサンドボックス外で安全に実行
- **引数検証**: Codex CLIから渡される引数は無視される
- **エラーハンドリング**: ファイル読み込みエラーを適切に処理

## 📚 参考資料

- [Codex公式ドキュメント](https://github.com/openai/codex)
- [PowerShell SoundPlayer](https://docs.microsoft.com/en-us/dotnet/api/system.media.soundplayer)
- [akademeia.info記事](https://akademeia.info/?p=43790)

## 🎉 まとめ

この機能により、長時間タスクの完了を逃すことなく、効率的に作業できます！

**利用シーン:**
- 🕒 長時間のコードレビュー
- 🔬 ディープリサーチタスク
- 🧪 大規模テスト生成
- 🚀 並列エージェント実行

音声通知で、Codexの完了を逃さず、次のタスクにスムーズに移行しましょう！🎊

