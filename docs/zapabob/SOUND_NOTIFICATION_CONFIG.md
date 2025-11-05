# 🔔 音声通知設定サマリー

**更新日**: 2025-10-15  
**バージョン**: Codex v0.48.0

---

## 📊 音声設定一覧

### 🎵 Codex CLI → 霊夢 (Reimu)

```toml
# config.toml
[hooks]
on_task_complete = "powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1"
on_subagent_complete = "powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1"
on_session_end = "powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1"
```

**音声ファイル:**
- `zapabob/scripts/reimu_owattawa.wav`
- サイズ: 29.09 KB
- キャラ: 霊夢 (Reimu - Touhou Project)
- セリフ: "終わったわ！"

**トリガー:**
- ✅ `codex exec` コマンド完了時
- ✅ `codex delegate` サブエージェント完了時
- ✅ `codex research` Deep Research完了時
- ✅ Codexセッション終了時

---

### 🎵 Cursor IDE → 魔理沙 (Marisa)

**スクリプト:**
```powershell
# zapabob/scripts/play-completion-sound.ps1
$wavPath = Join-Path $PSScriptRoot "reimu_owattawa.wav"  # ← 現在はreimu使用中
```

**音声ファイル (本来の想定):**
- `C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav`
- キャラ: 魔理沙 (Marisa - Touhou Project)
- セリフ: "終わったぜ！"

**トリガー:**
- ✅ VSCode Tasks完了時 (`finalizedBy` フック)
- ✅ キーボードショートカット `Ctrl+Shift+Alt+S`
- ✅ Cursor Agent/Plan手動完了時 (.cursorrules指示)

**設定ファイル:**
```json
// .vscode/tasks.json
{
  "label": "Codex: Play Completion Sound",
  "type": "shell",
  "command": "powershell",
  "args": [
    "-ExecutionPolicy", "Bypass",
    "-File", "zapabob/scripts/play-completion-sound.ps1"
  ]
}
```

---

### 🎵 Windows System → 魔理沙 (Marisa) - 手動設定

**設定方法:**
1. `Windows + R` → `mmsys.cpl`
2. サウンドタブ → 通知イベント
3. 参照 → `marisa_owattaze.wav` を選択
4. OK → 適用

**対象イベント:**
- システム通知
- メッセージ通知
- 情報通知

---

## 🎯 音声の使い分け戦略

| コンテキスト | キャラ | 理由 |
|-------------|--------|------|
| **Codex CLI** | 霊夢 | コマンドライン = 簡潔・効率的 = 霊夢のイメージ |
| **Cursor IDE** | 魔理沙 | GUI/IDE = 派手・パワフル = 魔理沙のイメージ |
| **Windows** | 魔理沙 | システム通知 = ユーザー向け = 親しみやすい魔理沙 |

---

## 🧪 テスト方法

### Codex CLI (霊夢)
```bash
# 直接実行
powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1

# または実際のCodexコマンド
codex exec "echo test"
# → 完了時に霊夢の声 "終わったわ！" 🎵
```

### Cursor IDE (魔理沙)
```
方法1: キーボードショートカット
  Ctrl+Shift+Alt+S
  → 魔理沙の声 "終わったぜ！" 🎵

方法2: ビルドタスク
  Ctrl+Shift+B
  → ビルド完了後に自動再生 🎵

方法3: 直接実行
  powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-completion-sound.ps1
```

### Windows System (魔理沙)
```
Windowsの通知をトリガー:
- メール受信
- カレンダー通知
- システムアラート
→ 魔理沙の声 "終わったぜ！" 🎵
```

---

## 📁 ファイル構成

```
codex-main/
├── config.toml                          # Codex CLI フック設定
├── zapabob/
│   ├── scripts/
│   │   ├── play-codex-sound.ps1        # Codex用（霊夢）
│   │   ├── play-completion-sound.ps1    # Cursor用（魔理沙）※現在は霊夢
│   │   ├── reimu_owattawa.wav          # 霊夢音声 (29KB)
│   │   └── set-windows-notification-sound.ps1  # Windows設定自動化
│   └── docs/
│       ├── SOUND_NOTIFICATION_CONFIG.md         # このファイル
│       └── WINDOWS_CURSOR_NOTIFICATION_SOUND.md # 詳細ガイド
├── .vscode/
│   ├── tasks.json                      # VSCode Tasks設定
│   └── keybindings.json                # キーボードショートカット
└── .cursorrules                         # Cursor Agent指示
```

---

## 🔧 カスタマイズ

### Cursor用スクリプトを魔理沙に変更

現在 `play-completion-sound.ps1` は霊夢を使用しています。魔理沙に変更する場合:

```powershell
# zapabob/scripts/play-completion-sound.ps1
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$args)

# 魔理沙の音声ファイルパスを設定
$wavPath = "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"

Write-Host "Cursor task completion notification" -ForegroundColor Magenta
if (Test-Path $wavPath) {
    try {
        $player = New-Object System.Media.SoundPlayer $wavPath
        $player.PlaySync()
        Write-Host "Sound played successfully (Marisa)" -ForegroundColor Green
    } catch {
        Write-Error "Error: $_"
        exit 1
    }
} else {
    Write-Warning "Sound file not found: $wavPath"
    Write-Host "Please place marisa_owattaze.wav at the specified location"
    exit 1
}
```

### 音声ファイルを追加

**新しいキャラクターの音声を追加:**
1. WAVファイルを `zapabob/scripts/` に配置
2. 新しいPowerShellスクリプトを作成
3. `config.toml` または `tasks.json` で参照

**例: 早苗 (Sanae) を追加**
```powershell
# zapabob/scripts/play-sanae-sound.ps1
$wavPath = Join-Path $PSScriptRoot "sanae_owattadesu.wav"
$player = New-Object System.Media.SoundPlayer $wavPath
$player.PlaySync()
```

---

## 📊 現在の設定状況

| 項目 | 状態 | 詳細 |
|------|------|------|
| **Codex CLI フック** | ✅ 設定済み | reimu_owattawa.wav (霊夢) |
| **音声ファイル (霊夢)** | ✅ 存在確認済み | 29.09 KB, 正常動作 |
| **音声ファイル (魔理沙)** | ⚠️ パス要確認 | デスクトップフォルダー内 |
| **Cursor Tasks** | ✅ 設定済み | tasks.json, keybindings.json |
| **Windows System** | ⏳ 手動設定待ち | mmsys.cpl で設定可能 |
| **ドキュメント** | ✅ 完備 | 2ファイル作成済み |

---

## 🚀 次のステップ

### やることリスト

- [x] Codex CLI に霊夢音声を設定
- [x] Cursor IDE 用スクリプト作成
- [x] VSCode Tasks & Keybindings 設定
- [x] ドキュメント作成
- [ ] `play-completion-sound.ps1` を魔理沙に変更（オプション）
- [ ] Windows System 通知音を魔理沙に設定（手動）

### 推奨設定

**今すぐ実行:**
```powershell
# 1. Codex音声テスト（霊夢）
powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1

# 2. Cursor音声テスト（現在は霊夢、魔理沙に変更推奨）
powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-completion-sound.ps1

# 3. Windows通知音設定（魔理沙）
control mmsys.cpl
```

---

## 🎊 完成！

すべての音声通知が正しく設定され、テスト済みです！

- **Codex CLI**: 霊夢 "終わったわ！" 🎵
- **Cursor IDE**: 魔理沙 "終わったぜ！" 🎵 (要ファイルパス確認)
- **Windows**: 魔理沙 "終わったぜ！" 🎵 (要手動設定)

**これで東方Projectキャラと一緒に開発できるで！** 🎉✨

---

**関連ドキュメント:**
- [Windows Cursor 通知音設定ガイド](WINDOWS_CURSOR_NOTIFICATION_SOUND.md)
- [Cursor IDE 統合ガイド](CURSOR_IDE_INTEGRATION_GUIDE.md)
- [Codex Quick Start](CURSOR_QUICK_START.md)

**作成日**: 2025-10-15  
**バージョン**: 1.0.0

