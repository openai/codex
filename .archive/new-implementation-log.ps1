# 🚀 実装ログ自動生成スクリプト
# Usage: .\scripts\new-implementation-log.ps1 "機能名"

param(
    [Parameter(Mandatory=$true)]
    [string]$FeatureName
)

$Date = Get-Date -Format "yyyy-MM-dd"
$Time = Get-Date -Format "HH:mm"
$Version = Get-Content -Path "VERSION" -ErrorAction SilentlyContinue
if (-not $Version) {
    $Version = "0.47.0-alpha.1"
}

$FileName = "_docs/${Date}_${FeatureName}.md"

$Template = @"
# 🚀 ${FeatureName} 実装完了

**実装日時**: ${Date} ${Time} JST  
**バージョン**: ${Version}  
**Status**: 🚧 進行中

---

## 📋 実装内容

### 目的
[なぜこの機能を実装したか]

### 変更ファイル
- ``path/to/file.rs``

### 主な変更点
1. [変更1]
2. [変更2]

---

## ✅ 完了条件チェック

- [ ] 実装完了
- [ ] テスト追加
- [ ] ドキュメント更新
- [ ] Clippy通過
- [ ] ビルド成功

---

## 🧪 テスト結果

``````bash
cargo test -p codex-xxx
# 結果を貼り付け
``````

---

## 📝 コミット情報

``````bash
git log --oneline -1
# コミットハッシュとメッセージ
``````

---

## 💡 今後の課題

- [課題1]
- [課題2]

---

**END OF IMPLEMENTATION LOG**
"@

# ファイル作成
Set-Content -Path $FileName -Value $Template -Encoding UTF8

Write-Host "✅ Created: $FileName" -ForegroundColor Green
Write-Host ""
Write-Host "次のステップ:" -ForegroundColor Cyan
Write-Host "  1. エディタで $FileName を開く" -ForegroundColor Yellow
Write-Host "  2. [実装内容] セクションを記入" -ForegroundColor Yellow
Write-Host "  3. テスト結果を貼り付け" -ForegroundColor Yellow
Write-Host "  4. コミット情報を追加" -ForegroundColor Yellow

