# 2025-10-23 Play Completion Sound Path Fix

## Summary
`play-completion-sound.ps1`スクリプトのパスを現在のプロジェクトディレクトリに動的に解決するように修正したで！

## 問題
ハードコードされたパスが古いプロジェクトの場所を指していた：
```powershell
$wavPath = "C:\Users\downl\Desktop\codex-main\codex-main\.codex\marisa_owattaze.wav"
```

実際のプロジェクトは`C:\Users\downl\Desktop\codex`にあるため、スクリプトが失敗していた。

## 解決方法
スクリプトのディレクトリから動的にプロジェクトルートを解決するように変更：

```powershell
# Get current script directory and resolve to project root
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$wavPath = Join-Path $projectRoot ".codex\marisa_owattaze.wav"
```

## 動作確認
```powershell
powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-completion-sound.ps1
```

**結果：**
```
🎉 Agent/Plan completion notification (Marisa)
Looking for sound file: C:\Users\downl\Desktop\codex\.codex\marisa_owattaze.wav
✅ Sound played: 終わったぜ！ (Marisa)
```

## 修正内容

### Before
```powershell
$wavPath = "C:\Users\downl\Desktop\codex-main\codex-main\.codex\marisa_owattaze.wav"
```

### After
```powershell
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$wavPath = Join-Path $projectRoot ".codex\marisa_owattaze.wav"
```

## メリット
1. ✅ プロジェクトの場所が変わっても自動的に適応
2. ✅ 相対パスで音声ファイルを解決
3. ✅ デバッグ情報を追加（音声ファイルの場所を表示）
4. ✅ より柔軟でメンテナンスしやすい

## 音声ファイルの場所
- プロジェクト内: `C:\Users\downl\Desktop\codex\.codex\marisa_owattaze.wav`
- バックアップ: `C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav`

## 変更ファイル
- `zapabob/scripts/play-completion-sound.ps1`

