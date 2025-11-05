# ビルド完了後の実行手順

ビルド完了（音声「終わったぜ！」が鳴った後）の手順

---

## ✅ Step 1: ビルド成果物確認

```powershell
# 実行ファイル確認
ls .\src-tauri\target\release\codex-tauri.exe

# サイズ確認
$exe = Get-Item .\src-tauri\target\release\codex-tauri.exe
Write-Host "Size: $([math]::Round($exe.Length / 1MB, 2)) MB"
```

**期待**: ~15-30MB

---

## 🚀 Step 2: 実行（デバッグモード）

```powershell
# 直接実行（インストール不要）
.\src-tauri\target\release\codex-tauri.exe
```

**確認項目**:
- ✅ ウィンドウが表示される
- ✅ システムトレイにアイコンが表示される
- ✅ Dashboardが正常に表示される

---

## 🔒 Step 3: セキュリティテスト

```powershell
# 自動セキュリティチェック実行
.\test-security.ps1
```

**チェック内容**:
1. バイナリ存在確認
2. ファイルサイズ確認
3. CSP設定検証
4. Shell実行禁止確認
5. npm audit
6. cargo audit  
7. ファイル権限確認
8. コード署名状態
9. プロセス権限確認
10. メモリ安全性確認

**期待結果**:
```
✅ すべてのテストに合格しました！
合格: 10 / 10
```

---

## 🎯 Step 4: 機能テスト

### 4-1. ファイル監視テスト

1. Dashboard → ワークスペースパス入力
   ```
   C:\Users\downl\Desktop\codex
   ```

2. 「Start Monitoring」クリック

3. テストファイル作成
   ```powershell
   cd C:\Users\downl\Desktop\codex
   echo "test" > test-file.txt
   ```

4. Dashboard → Recent Changesに表示されることを確認

5. デスクトップ通知が表示されることを確認

### 4-2. システムトレイテスト

1. システムトレイアイコン右クリック
2. メニュー表示確認
3. 「⚙️ Settings」クリック → Settings画面表示
4. 「📖 Docs」クリック → ブラウザでGitHub開く
5. 左クリック → ウィンドウ非表示
6. もう一度左クリック → ウィンドウ表示

### 4-3. Blueprintテスト

1. Dashboard → 「📋 New Blueprint」ボタンクリック
2. Description入力: "Add logging functionality"
3. OK → アラート「Blueprint created successfully!」表示確認

### 4-4. カーネルステータステスト

1. Dashboard画面をスクロール
2. 「AIネイティブOS - カーネル統合」セクション確認
3. 「❌ ドライバー未起動」バッジ確認（シミュレーションモード）
4. GPUステータス表示確認（シミュレーションデータ）

---

## 📊 Step 5: パフォーマンステスト

### メモリ使用量

```powershell
# タスクマネージャーで確認または:
Get-Process codex-tauri | Select-Object ProcessName, @{Name="Memory(MB)";Expression={[math]::Round($_.WorkingSet / 1MB, 2)}}
```

**目標**: < 150MB

### CPU使用率

タスクマネージャーで「codex-tauri.exe」を確認

**目標（アイドル時）**: < 1%

### 起動時間

```powershell
Measure-Command {
    Start-Process .\src-tauri\target\release\codex-tauri.exe
    Start-Sleep -Seconds 2  # ウィンドウ表示待ち
}
```

**目標**: < 2秒

---

## 🔧 Step 6: 詳細セキュリティテスト（オプション）

### Process Monitor

1. [Process Monitor](https://docs.microsoft.com/en-us/sysinternals/downloads/procmon)ダウンロード
2. Procmon.exe起動
3. フィルター: Process Name = codex-tauri.exe
4. Codex起動して操作
5. 確認:
   - ✅ `%APPDATA%\codex`以外へのファイルアクセスなし
   - ✅ HKEY_CURRENT_USER以外へのレジストリ書き込みなし

### Wireshark

1. [Wireshark](https://www.wireshark.org/)ダウンロード
2. キャプチャ開始
3. フィルター: `ip.addr != 127.0.0.1 && tcp`
4. Codex操作
5. 確認:
   - ✅ 外部サーバーへの不明な通信なし

---

## 💾 Step 7: MSIインストーラー作成（オプション）

Tauri bundlerでMSI作成:

```powershell
cd ..
npx tauri build
```

**出力先**: `src-tauri\target\release\bundle\msi\Codex_0.1.0_x64.msi`

**インストール**:
```powershell
$msi = Get-ChildItem .\src-tauri\target\release\bundle\msi\*.msi | Select-Object -First 1
msiexec /i "$($msi.FullName)" /qb
```

---

## 🎉 完了！

すべてのテストに合格したら、Codex AI-Native OSの準備完了や！

**次のアクション**:
- 実際のプロジェクトで使用開始
- フィードバック収集
- カーネルドライバー統合（将来）

---

**作成日**: 2025-11-03
**ステータス**: Ready for Testing 🚀

