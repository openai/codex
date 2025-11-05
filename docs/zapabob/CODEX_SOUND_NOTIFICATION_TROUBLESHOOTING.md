# Codex 音声通知トラブルシューティング

**更新日**: 2025-10-16  
**バージョン**: v0.48.0

---

## 🔊 音声通知システム

Codexには2つの音声通知があります：

| 用途 | 音声ファイル | スクリプト | 使用場面 |
|------|-------------|-----------|----------|
| **Codex CLI** | `reimu_owattawa.wav` | `play-codex-sound.ps1` | Codex CLIタスク完了時 |
| **Cursor IDE** | `marisa_owattaze.wav` | `play-completion-sound.ps1` | Cursor Agent/Plan完了時 |

---

## ❌ 問題: Codex CLIで音声が再生されない

### 症状
```bash
codex exec "echo test"
# → 音声が鳴らない
```

### 原因

#### 1. **相対パスの問題** ✅ 修正済み
**問題**: `config.toml`のフックが相対パスを使用していた
```toml
# ❌ NG: 相対パス（カレントディレクトリに依存）
on_task_complete = "powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-codex-sound.ps1"
```

**解決**: v0.48.0で絶対パスに変更
```toml
# ✅ OK: 絶対パス
on_task_complete = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
```

#### 2. **`codex exec`はフックを発火しない**
**問題**: `codex exec`は非対話モードのため、一部のフックイベントが発火しない可能性

**対象フック**:
- ❌ `on_task_complete` - 発火しない可能性
- ❌ `on_subagent_complete` - 発火しない可能性
- ⚠️ `on_session_end` - セッション終了時のみ

**推奨**: 対話型TUIモード（`codex`）を使用

```bash
# ✅ 推奨: 対話型モード（フックが動作）
codex
> echo test

# ❌ 非推奨: 非対話モード（フックが動作しない可能性）
codex exec "echo test"
```

---

## ✅ 検証方法

### 1. 音声ファイルの存在確認
```powershell
# Reimu音声ファイル
Test-Path C:\Users\downl\Desktop\codex-main\codex-main\zapabob\scripts\reimu_owattawa.wav
# → True であること

# Marisa音声ファイル
Test-Path "C:\Users\downl\Desktop\新しいフォルダー (4)\marisa_owattaze.wav"
# → True であること
```

### 2. スクリプトの手動実行
```powershell
# Reimu音声（Codex用）
powershell -ExecutionPolicy Bypass -File C:\Users\downl\Desktop\codex-main\codex-main\zapabob\scripts\play-codex-sound.ps1
# → "Sound played successfully (Reimu)" と表示されること

# Marisa音声（Cursor用）
powershell -ExecutionPolicy Bypass -File C:\Users\downl\Desktop\codex-main\codex-main\zapabob\scripts\play-completion-sound.ps1
# → "Sound played successfully (Marisa)" と表示されること
```

### 3. Config.tomlの確認
```bash
# フック設定を確認
grep -A 10 "\[hooks\]" config.toml
```

**期待される出力**:
```toml
[hooks]
# タスク全体完了時（Plan実行完了時） → Reimu
on_task_complete = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
# サブエージェント完了時（Agent実行完了時） → Reimu
on_subagent_complete = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
# セッション終了時 → Reimu
on_session_end = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
```

---

## 🔧 修正手順

### Step 1: 絶対パスへの変更（v0.48.0で実施済み）

`config.toml`を編集：
```toml
[hooks]
on_task_complete = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
on_subagent_complete = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
on_session_end = "powershell -ExecutionPolicy Bypass -File C:\\Users\\downl\\Desktop\\codex-main\\codex-main\\zapabob\\scripts\\play-codex-sound.ps1"
```

### Step 2: 対話型モードでテスト

```bash
# 対話型TUIを起動
codex

# プロンプトで簡単なコマンド実行
> echo "test"

# セッション終了（Ctrl+D または exit）
> exit

# → Reimu音声が再生されるはず
```

### Step 3: デバッグ（音声が鳴らない場合）

1. **フックログを確認**
   ```bash
   # 監査ログディレクトリを確認
   ls ~/.codex/audit-logs/
   
   # 最新ログを確認
   cat ~/.codex/audit-logs/latest.json | grep hook
   ```

2. **手動でフックを実行**
   ```powershell
   # Codexが実際に実行するコマンドをそのまま実行
   powershell -ExecutionPolicy Bypass -File C:\Users\downl\Desktop\codex-main\codex-main\zapabob\scripts\play-codex-sound.ps1
   
   # エラーが出る場合、パスを確認
   ```

3. **音量設定を確認**
   - Windowsのボリュームミキサーでpowershell.exeがミュートされていないか確認
   - システムボリュームが0でないか確認

---

## 📝 既知の制限事項

### 1. `codex exec`での音声通知
- **制限**: `codex exec`コマンドは非対話モードのため、フックイベントが発火しない可能性が高い
- **回避策**: 対話型TUIモード（`codex`）を使用する

### 2. バックグラウンド実行
- **制限**: Codexをバックグラウンドで実行した場合、音声が聞こえない可能性
- **回避策**: フォアグラウンドで実行する

### 3. WSL/Linux環境
- **制限**: Windows音声APIを使用するため、WSL/Linux環境では動作しない
- **回避策**: Windowsネイティブ環境で実行する

---

## 🎯 推奨使用方法

### Codex CLI（Reimu音声）
```bash
# ✅ 推奨: 対話型TUI
codex
> codex delegate code-reviewer --scope ./src
> exit  # → Reimu音声が再生

# ✅ 推奨: サブエージェント
codex agent "Review this code"
# → 完了時にReimu音声が再生

# ❌ 非推奨: codex exec（フックが動かない）
codex exec "echo test"  # 音声なし
```

### Cursor IDE（Marisa音声）
```
1. Cursor Agentでタスク実行
2. Agentがタスク完了を認識
3. Agentが play-completion-sound.ps1 を実行提案
4. ユーザーが承認
5. → Marisa音声が再生
```

または

```
Ctrl+Shift+Alt+S  # 手動でMarisa音声再生
```

---

## 🔗 関連ドキュメント

- [音声通知設定ガイド](CURSOR_SOUND_NOTIFICATION.md)
- [Windows通知音変更手順](WINDOWS_CURSOR_NOTIFICATION_SOUND.md)
- [Codex設定リファレンス](../config.toml)

---

## 💡 FAQ

**Q: Reimu音声とMarisa音声の使い分けは？**
A: 
- **Reimu** (`reimu_owattawa.wav`): Codex CLIのタスク完了時
- **Marisa** (`marisa_owattaze.wav`): Cursor IDEのAgent/Plan完了時

**Q: 音声を無効にするには？**
A: `config.toml`の`[hooks]`セクションをコメントアウトまたは削除

```toml
# [hooks]
# on_task_complete = "..."
```

**Q: 別の音声ファイルを使いたい**
A: 
1. 新しい`.wav`ファイルを用意
2. スクリプトの`$wavPath`を変更
3. `config.toml`の`[hooks]`を更新

---

**作成日**: 2025-10-16  
**作成者**: AI Assistant (Claude Sonnet 4.5)  
**バージョン**: v0.48.0

