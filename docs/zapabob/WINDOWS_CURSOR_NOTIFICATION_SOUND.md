# 🔔 WindowsでCursorの通知音を変更する方法

**対象**: Windows 11/10  
**アプリ**: Cursor IDE  
**音声**: marisa_owattaze.wav

---

## 📋 方法1: Windowsシステム通知音を変更（推奨）

### 手順

#### Step 1: サウンド設定を開く

**方法A: 設定アプリから**
```
1. Windowsキー + I を押す
2. 「システム」→「サウンド」をクリック
3. 右側の「詳細設定」または「サウンドの詳細設定」をクリック
```

**方法B: コントロールパネルから**
```
1. Windowsキー + R を押す
2. "mmsys.cpl" と入力してEnter
3. サウンド設定が開く
```

**方法C: PowerShellコマンド**
```powershell
# サウンド設定を開く
control mmsys.cpl
```

#### Step 2: 通知音を変更

```
1. 「サウンド」タブを選択
2. 「プログラムイベント」リストから以下を探す：
   - 「通知」
   - 「メッセージ (情報)」
   - 「システム通知」
3. イベントを選択
4. 下部の「サウンド」ドロップダウンをクリック
5. 「参照」ボタンをクリック
6. marisa_owattaze.wav を選択
   パス: C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav
7. 「OK」をクリック
```

#### Step 3: テスト

```
1. 「テスト」ボタンをクリック
2. 魔理沙の声が聞こえればOK！🎵
3. 「適用」→「OK」で保存
```

---

## 📋 方法2: PowerShellで自動設定

### 自動設定スクリプト

**ファイル**: `zapabob/scripts/set-windows-notification-sound.ps1`

```powershell
# Windows通知音を変更するスクリプト
param(
    [string]$SoundFile = "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"
)

Write-Host "Setting Windows notification sound..." -ForegroundColor Cyan

# 音声ファイルの存在確認
if (-not (Test-Path $SoundFile)) {
    Write-Error "Sound file not found: $SoundFile"
    exit 1
}

# レジストリパス
$regPath = "HKCU:\AppEvents\Schemes\Apps\.Default\Notification.Default\.Current"

# 現在の設定をバックアップ
$currentSound = Get-ItemProperty -Path $regPath -Name "(Default)" -ErrorAction SilentlyContinue
if ($currentSound) {
    Write-Host "Current sound: $($currentSound.'(Default)')" -ForegroundColor Gray
}

# 新しい音声を設定
try {
    Set-ItemProperty -Path $regPath -Name "(Default)" -Value $SoundFile
    Write-Host "SUCCESS: Notification sound updated!" -ForegroundColor Green
    Write-Host "New sound: $SoundFile" -ForegroundColor White
    
    # テスト再生
    Write-Host "`nTesting sound..." -ForegroundColor Yellow
    $player = New-Object System.Media.SoundPlayer $SoundFile
    $player.PlaySync()
    Write-Host "Sound test complete!" -ForegroundColor Green
} catch {
    Write-Error "Failed to set notification sound: $_"
    exit 1
}
```

### 実行方法

```powershell
# 管理者権限で実行（推奨）
powershell -ExecutionPolicy Bypass -File zapabob/scripts/set-windows-notification-sound.ps1

# または特定のファイルを指定
powershell -ExecutionPolicy Bypass -File zapabob/scripts/set-windows-notification-sound.ps1 -SoundFile "path\to\your.wav"
```

---

## 📋 方法3: Cursor固有の通知設定

### Cursor設定ファイル

Cursorには独自の通知音設定はありませんが、VS Code互換の設定で音声を鳴らせます。

**設定ファイル**: `.vscode/settings.json`

```json
{
  "window.titleBarStyle": "custom",
  "window.enableMenuBarMnemonics": false,
  
  // 通知設定
  "files.autoSave": "afterDelay",
  "files.autoSaveDelay": 1000,
  
  // タスク完了時の音声通知（カスタム）
  "tasks.problemMatchers.showNotifications": true
}
```

---

## 🎯 Cursor特化: タスク完了音声通知

既に設定済みの方法：

### キーボードショートカット
```
Ctrl+Shift+Alt+S
→ 魔理沙の音声再生
```

### VSCode Tasks
```json
{
  "label": "任意のタスク",
  "finalizedBy": ["Codex: Play Completion Sound"]
}
```

### .cursorrules指示
Cursor Agentが自動で音声を再生（設定済み）

---

## 🔧 トラブルシューティング

### 音が鳴らない場合

#### 確認1: 音声ファイルの存在
```powershell
Test-Path "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"
```

#### 確認2: 音量設定
```
1. タスクバーのスピーカーアイコンを右クリック
2. 「音量ミキサーを開く」
3. 「システムサウンド」の音量を確認
```

#### 確認3: Windowsサウンドスキーム
```
1. サウンド設定を開く
2. 「サウンド スキーム」が「(変更なし)」または「Windows既定」になっているか確認
```

#### 確認4: 手動テスト
```powershell
# PowerShellで直接再生
$player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"
$player.PlaySync()
```

---

## 📊 音声ファイル要件

### WAVファイル仕様
- **フォーマット**: WAV (PCM)
- **サンプリングレート**: 8000Hz - 48000Hz
- **ビット深度**: 8-bit / 16-bit
- **チャンネル**: モノラル or ステレオ
- **推奨サイズ**: < 1MB

### 現在の設定

#### Codex CLI
```
ファイル: reimu_owattawa.wav
パス: zapabob/scripts/reimu_owattawa.wav
用途: Codex CLI フック通知音
キャラクター: 霊夢 (Reimu - Touhou Project)
フック: on_task_complete, on_subagent_complete, on_session_end
```

#### Cursor IDE
```
ファイル: marisa_owattaze.wav
パス: C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav
用途: Cursor IDE タスク通知音
キャラクター: 魔理沙 (Marisa - Touhou Project)
トリガー: VSCode Tasks, キーボードショートカット
```

#### Windows System
```
ファイル: marisa_owattaze.wav (推奨)
パス: 手動設定（任意の場所）
用途: Windows システム通知音
設定方法: mmsys.cpl から手動設定
```

---

## 🎯 まとめ

### 3つの音声通知レベル

| レベル | 対象 | 音声 | キャラ | 設定方法 |
|--------|------|------|--------|---------|
| **1. Windowsシステム** | すべてのアプリ通知 | marisa_owattaze.wav | 魔理沙 | システム設定 or レジストリ |
| **2. Cursor Tasks** | VSCodeタスク完了 | marisa_owattaze.wav | 魔理沙 | tasks.json (設定済み) |
| **3. Codex CLI** | Codexコマンド完了 | **reimu_owattawa.wav** | **霊夢** | config.toml (設定済み) |

**重要**: Codex CLIは霊夢（Reimu）の音声を使用します！

### クイックアクセス

**すぐに音声を鳴らす:**
```
Ctrl+Shift+Alt+S
```

**Windowsシステム通知音設定:**
```
Win + R → mmsys.cpl → Enter
```

**PowerShellで設定:**
```powershell
powershell -ExecutionPolicy Bypass -File zapabob/scripts/set-windows-notification-sound.ps1
```

---

## 🚀 次のステップ

1. ✅ Windowsシステム通知音を変更（方法1 or 2）
2. ✅ Cursor Tasksで自動再生確認（Ctrl+Shift+B）
3. ✅ キーボードショートカットテスト（Ctrl+Shift+Alt+S）
4. ✅ Cursor Agentで動作確認

**これで完璧！すべての通知が魔理沙の声になるで！** 🎵✨

---

**参考リンク:**
- [Windows サウンド設定](https://support.microsoft.com/ja-jp/windows)
- [レジストリエディタでの音声設定](https://learn.microsoft.com/ja-jp/windows/win32/multimedia/system-sounds)
- [VS Code Tasks](https://code.visualstudio.com/docs/editor/tasks)

