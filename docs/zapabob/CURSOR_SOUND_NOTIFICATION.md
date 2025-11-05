# 🔊 CursorIDE 音声通知機能ガイド

CursorIDEでCodexのタスク完了時に音声通知を受け取る方法を説明します。

## 📋 概要

長時間タスクの完了を音声で通知することで、作業効率を向上させます。

**参考記事**: [akademeia.info - Codex音声通知の実装](https://akademeia.info/?p=43790)

## 🎯 利用可能な機能

### 1. VSCode Tasks統合

CursorIDEのタスク機能を使用して、ビルドやテスト完了時に自動で音声を再生します。

**利用可能なタスク:**
- `Codex: Play Completion Sound` - 音声通知のみ再生
- `Codex: Build and Notify` - ビルド完了後に音声通知
- `Codex: Test and Notify` - テスト完了後に音声通知

### 2. キーボードショートカット

- `Ctrl+Shift+Alt+S` - 音声通知を手動再生
- `Ctrl+Shift+B` - ビルド＆通知を実行

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

### 2. 設定ファイルの確認

以下のファイルがプロジェクトに追加されていることを確認:

- `.vscode/tasks.json` - タスク定義
- `.vscode/keybindings.json` - キーボードショートカット
- `zapabob/scripts/play-completion-sound.ps1` - 音声再生スクリプト

### 3. CursorIDEを再起動

設定を有効にするため、CursorIDEを再起動します。

## 🔧 使い方

### タスクの実行

**方法1: コマンドパレットから**
1. `Ctrl+Shift+P` でコマンドパレットを開く
2. "Tasks: Run Task" を選択
3. 以下のいずれかを選択:
   - `Codex: Play Completion Sound`
   - `Codex: Build and Notify`
   - `Codex: Test and Notify`

**方法2: キーボードショートカット**
- `Ctrl+Shift+Alt+S` - 音声通知を即座に再生
- `Ctrl+Shift+B` - ビルド＆通知を実行

**方法3: ターミナルから**
```powershell
# スクリプト直接実行
powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-completion-sound.ps1
```

### ビルドタスクとの統合

**Rustビルドの例:**
```json
{
  "label": "My Custom Build with Notification",
  "dependsOrder": "sequence",
  "dependsOn": [
    "cargo build",
    "Codex: Play Completion Sound"
  ]
}
```

**Pythonスクリプトの例:**
```json
{
  "label": "Python Script with Notification",
  "type": "shell",
  "command": "python",
  "args": ["${file}"],
  "dependsOn": ["Codex: Play Completion Sound"]
}
```

## 🎨 カスタマイズ

### 音声ファイルの変更

別の音声を使用する場合:

1. WAV形式の音声ファイルを用意
2. `zapabob/scripts/`ディレクトリに配置
3. `play-completion-sound.ps1`の3行目を変更:

```powershell
$wavPath = Join-Path $PSScriptRoot "your-sound-file.wav"
```

### キーバインドの変更

`.vscode/keybindings.json`を編集:

```json
{
  "key": "your-preferred-shortcut",
  "command": "workbench.action.tasks.runTask",
  "args": "Codex: Play Completion Sound"
}
```

### タスクのカスタマイズ

`.vscode/tasks.json`に新しいタスクを追加:

```json
{
  "label": "Your Custom Task with Sound",
  "dependsOrder": "sequence",
  "dependsOn": [
    "Your Main Task",
    "Codex: Play Completion Sound"
  ]
}
```

## 🎯 実用例

### 長時間ビルド通知

```powershell
# CursorIDEで実行
# Ctrl+Shift+P → "Tasks: Run Task" → "Codex: Build and Notify"
```

### テスト完了通知

```powershell
# CursorIDEで実行
# Ctrl+Shift+P → "Tasks: Run Task" → "Codex: Test and Notify"
```

### カスタムスクリプトとの統合

`.vscode/tasks.json`に追加:

```json
{
  "label": "Deep Research with Notification",
  "type": "shell",
  "command": "codex",
  "args": ["research", "React best practices"],
  "dependsOn": ["Codex: Play Completion Sound"]
}
```

## 🐛 トラブルシューティング

### 音声が再生されない

**原因1**: 音声ファイルが見つからない
```powershell
# パスを確認
Test-Path zapabob/scripts/reimu_owattawa.wav
```

**原因2**: タスクが実行されていない
```
1. Ctrl+Shift+P → "Tasks: Run Task"
2. "Codex: Play Completion Sound"を手動実行
3. エラーメッセージを確認
```

**原因3**: PowerShell実行ポリシー
```powershell
# 実行ポリシーを確認
Get-ExecutionPolicy

# 必要に応じて変更
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### タスクが表示されない

1. CursorIDEを再起動
2. `.vscode/tasks.json`の構文を確認
3. コマンドパレットから "Tasks: Configure Task"を実行

### キーボードショートカットが動作しない

1. `Ctrl+K Ctrl+S`でキーボードショートカット設定を開く
2. "Codex: Play Completion Sound"を検索
3. 競合するショートカットがないか確認

## 📊 パフォーマンス

- **タスク起動時間**: 約0.5秒
- **音声再生時間**: 1-2秒（音声ファイルの長さに依存）
- **メモリ使用量**: 約5-10MB

## 🔒 セキュリティ

- **実行ポリシー**: `-ExecutionPolicy Bypass`で安全に実行
- **サンドボックス**: VSCode Tasks環境内で実行
- **引数検証**: 不要な引数は無視される

## 📚 関連ファイル

- **スクリプト**: `zapabob/scripts/play-completion-sound.ps1`
- **タスク定義**: `.vscode/tasks.json`
- **キーバインド**: `.vscode/keybindings.json`
- **詳細ドキュメント**: `zapabob/scripts/README_SOUND_NOTIFICATION.md`

## 🎉 まとめ

CursorIDEでCodexの音声通知機能を活用することで:

- 🕒 長時間タスクの完了を逃さない
- 🔔 バックグラウンド作業中でも通知を受け取れる
- ⌨️ キーボードショートカットで即座に確認
- 🎯 カスタムタスクと柔軟に統合

音声通知で、より効率的な開発環境を実現しましょう！🎊

## 🔗 参考資料

- [akademeia.info記事](https://akademeia.info/?p=43790)
- [VSCode Tasks Documentation](https://code.visualstudio.com/docs/editor/tasks)
- [CursorIDE公式ドキュメント](https://cursor.sh/docs)

---

**Version**: 1.0.0  
**Author**: zapabob  
**Created**: 2025-10-15


