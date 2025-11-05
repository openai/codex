# Codex Tauri - Quick Start Guide

インストール完了後の動作確認とセキュリティテスト手順

---

## ✅ インストール完了後の確認

### 1. システムトレイアイコン確認

Windows画面右下のタスクバーを確認：
- Codexアイコンが表示されているはず
- 表示されていない場合：
  - タスクバーの「^」（非表示アイコン）をクリック
  - または、スタートメニューから「Codex」を検索

### 2. アプリケーション起動

**方法 A**: システムトレイアイコンをクリック
- 左クリック: ウィンドウ表示/非表示
- 右クリック: メニュー表示 → 「📊 Dashboard を開く」

**方法 B**: スタートメニュー
- Windowsキー押下
- 「Codex」と入力
- Enterキー

### 3. 初期設定

#### 3-1. ワークスペース設定

1. Dashboard画面を開く
2. 「File System Watcher」セクションへ移動
3. ワークスペースパスを入力（例: `C:\Users\YourName\Projects\myproject`）
4. 「Start Monitoring」ボタンをクリック

#### 3-2. 自動起動設定（オプション）

1. Settings画面を開く
2. 「Auto-start on Windows boot」をON
3. 次回Windows起動時から自動起動

---

## 🔒 セキュリティテスト実行

### クイックテスト（1分）

```powershell
cd C:\Users\downl\Desktop\codex\codex-tauri
.\test-security.ps1
```

**チェック項目**:
- ✅ バイナリ存在確認
- ✅ ファイルサイズ確認
- ✅ Tauri設定検証（CSP/Shell制限）
- ✅ 依存関係脆弱性スキャン
- ✅ ファイル権限確認
- ✅ コード署名状態

**期待される出力**:
```
🔒 Codex Tauri セキュリティテスト
=================================

📦 Test 1: バイナリ確認
✅ バイナリ存在確認
✅ バイナリサイズ適正

⚙️  Test 2: Tauri設定確認
✅ CSP設定
✅ Shell実行禁止

...

📊 テスト結果サマリー
合格: 10 / 10

✅ すべてのテストに合格しました！
```

### 詳細テスト（5-10分）

`SECURITY_TEST.md`の手順に従って実行：

#### 1. Process Monitor（ファイル/レジストリ監視）

```powershell
# Download: https://docs.microsoft.com/en-us/sysinternals/downloads/procmon
# Run: Procmon.exe
# Filter: Process Name = Codex.exe
```

**確認事項**:
- ✅ `%APPDATA%\codex`以外へのファイルアクセスなし
- ✅ `HKEY_CURRENT_USER`以外へのレジストリ書き込みなし
- ✅ `C:\Windows\System32`へのアクセスなし

#### 2. Wireshark（ネットワーク監視）

```powershell
# Download: https://www.wireshark.org/
# Run: Wireshark
# Capture: Ethernet/Wi-Fi
# Filter: ip.addr != 127.0.0.1 && tcp
```

**確認事項**:
- ✅ ローカルホスト（127.0.0.1）通信のみ
- ✅ 外部サーバーへの不明な通信なし

#### 3. Process Explorer（プロセス/メモリ監視）

```powershell
# Download: https://docs.microsoft.com/en-us/sysinternals/downloads/process-explorer
# Run: procexp.exe (管理者権限)
# Find: Codex.exe
```

**確認事項**:
- ✅ Integrity Level: Medium（管理者権限なし）
- ✅ Memory: < 150MB
- ✅ CPU（アイドル）: < 1%

---

## 🎯 動作確認テスト

### Test 1: ファイル監視機能

1. Dashboard → ワークスペースパス入力
2. 「Start Monitoring」クリック
3. 監視対象ディレクトリでファイル編集（例: `test.txt`作成）
4. Dashboard → Recent Changesに表示されることを確認
5. デスクトップ通知が表示されることを確認（50行以上変更時）

### Test 2: Blueprint作成

1. Dashboard → 「📋 New Blueprint」ボタンクリック
2. Description入力（例: "Add logging functionality"）
3. OKクリック
4. アラート「Blueprint created successfully!」表示確認

### Test 3: Deep Research

1. Dashboard → 「🔍 Deep Research」ボタンクリック
2. Query入力（例: "React best practices 2025"）
3. OKクリック
4. コンソールに結果表示確認（F12開発者ツール）

### Test 4: システムトレイ操作

1. システムトレイアイコン右クリック
2. メニュー表示確認
3. 「⚙️ Settings」クリック → Settings画面表示
4. 「📖 Docs」クリック → ブラウザでGitHub開く
5. 左クリック → ウィンドウ非表示
6. もう一度左クリック → ウィンドウ表示

### Test 5: カーネルステータス（シミュレーション）

1. Dashboard画面をスクロール
2. 「AIネイティブOS - カーネル統合」セクション確認
3. 「❌ ドライバー未起動」バッジ確認
4. 説明文表示確認
5. 「ドライバーインストール（未実装）」ボタン確認

---

## 📊 パフォーマンステスト

### メモリ使用量測定

```powershell
# タスクマネージャーで確認
taskmgr

# または PowerShellで確認
Get-Process Codex | Select-Object ProcessName, @{Name="Memory(MB)";Expression={[math]::Round($_.WorkingSet / 1MB, 2)}}
```

**目標**: < 150MB

### CPU使用率測定

```powershell
# リソースモニター起動
perfmon /res

# Codexプロセスを確認
```

**目標（アイドル時）**: < 1%

### 起動時間測定

```powershell
Measure-Command {
    Start-Process "Codex" -Wait
}
```

**目標**: < 2秒

---

## 🚨 トラブルシューティング

### Issue 1: システムトレイアイコンが表示されない

**Solution**:
```powershell
# Explorerを再起動
Stop-Process -Name explorer -Force
Start-Process explorer
```

### Issue 2: ファイル監視が開始しない

**Check**:
1. ワークスペースパスが正しいか確認
2. パスが存在するか確認
3. 読み取り権限があるか確認

**Solution**:
```powershell
# パス存在確認
Test-Path "C:\path\to\workspace"

# 権限確認
icacls "C:\path\to\workspace"
```

### Issue 3: データベースエラー

**Solution**:
```powershell
# データベースディレクトリ確認
$dbDir = "$env:APPDATA\codex"
if (-not (Test-Path $dbDir)) {
    New-Item -ItemType Directory -Force -Path $dbDir
}

# 権限確認
icacls $dbDir
```

### Issue 4: アプリケーションが起動しない

**Check Event Log**:
```powershell
# イベントログ確認
Get-EventLog -LogName Application -Source "Codex" -Newest 10
```

**Check Logs**:
```powershell
# アプリログ確認（もしあれば）
cat "$env:APPDATA\codex\logs\*.log"
```

---

## 🎉 次のステップ

### セキュリティテスト合格後:

1. **実際のプロジェクトで使用開始**
   - 実際の開発プロジェクトをワークスペースに設定
   - ファイル変更を監視
   - Blueprint機能を活用

2. **カスタマイズ**
   - Settings → 監視除外パターン追加
   - テーマ変更（Light/Dark）
   - 通知設定調整

3. **カーネルドライバー統合（将来）**
   - 管理者権限でドライバーインストール
   - AIネイティブOS機能の有効化
   - パフォーマンス向上の確認

---

## 📞 サポート

問題が発生した場合:

1. **ログ確認**: `%APPDATA%\codex\`
2. **セキュリティテスト結果**: `security-test-results.json`
3. **インストールログ**: `install-log.txt`
4. **GitHub Issues**: https://github.com/zapabob/codex/issues

---

**作成日**: 2025-11-03  
**バージョン**: v0.1.0  
**ステータス**: Ready for Testing 🚀

