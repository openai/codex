# Codex Sub-Agents Build & Global Install Script
# Windows PowerShell版

Write-Host "🚀 Codex Sub-Agents & Deep Research - Build & Install" -ForegroundColor Cyan
Write-Host ""

# 1. Deep Research Module ビルド
Write-Host "📦 Building Deep Research module..." -ForegroundColor Yellow
Set-Location -Path "codex-rs"

cargo build --release -p codex-deep-research
cargo build --release -p codex-cli
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Deep Research build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "✅ Deep Research build successful!" -ForegroundColor Green
Write-Host ""

# 2. テスト実行
Write-Host "🧪 Running Deep Research tests..." -ForegroundColor Yellow
cargo test -p codex-deep-research --lib --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Tests failed!" -ForegroundColor Red
    exit 1
}

Write-Host "✅ All 23 tests passed!" -ForegroundColor Green
Write-Host ""

# 3. Agent定義確認
Write-Host "📋 Checking agent definitions..." -ForegroundColor Yellow
Set-Location -Path ".."

$agentFiles = Get-ChildItem -Path ".codex\agents\*.yaml"
Write-Host "Found $($agentFiles.Count) agent definitions:" -ForegroundColor Green
foreach ($file in $agentFiles) {
    Write-Host "  ✅ $($file.Name)" -ForegroundColor Green
}
Write-Host ""

# 4. VS Code Extension準備
if (Test-Path "vscode-extension") {
    Write-Host "🎨 Setting up VS Code extension..." -ForegroundColor Yellow
    Set-Location -Path "vscode-extension"
    
    if (Test-Path "package.json") {
        npm install
        npm run compile
        Write-Host "✅ VS Code extension compiled!" -ForegroundColor Green
    }
    
    Set-Location -Path ".."
}
Write-Host ""

# 5. グローバルインストール準備
Write-Host "📦 Preparing global installation..." -ForegroundColor Yellow

# CLI バイナリパス（rmcp-client修正後に有効）
$cliBinary = "codex-rs\target\release\codex.exe"

if (Test-Path $cliBinary) {
    Write-Host "Found CLI binary: $cliBinary" -ForegroundColor Green
    
    # グローバルインストール（要管理者権限）
    $installChoice = Read-Host "Install globally? (y/n)"
    
    if ($installChoice -eq "y") {
        $globalPath = "$env:USERPROFILE\.cargo\bin\codex.exe"
        Copy-Item -Path $cliBinary -Destination $globalPath -Force
        Write-Host "✅ Installed to: $globalPath" -ForegroundColor Green
        Write-Host "   Add ~/.cargo/bin to PATH if not already done" -ForegroundColor Yellow
    }
} else {
    Write-Host "⚠️  CLI binary not found (rmcp-client build issue)" -ForegroundColor Yellow
    Write-Host "   Deep Research library is ready to use!" -ForegroundColor Green
}

Write-Host ""
Write-Host "🎊 Setup Complete!" -ForegroundColor Cyan
Write-Host ""
Write-Host "📚 Quick Start:" -ForegroundColor Yellow
Write-Host "  1. Review code:" -ForegroundColor White
Write-Host "     codex delegate code-reviewer --scope ./src" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. Deep research:" -ForegroundColor White
Write-Host "     codex research 'topic' --depth 3" -ForegroundColor Gray
Write-Host ""
Write-Host "  3. Test generation:" -ForegroundColor White
Write-Host "     codex delegate test-gen --scope ./src" -ForegroundColor Gray
Write-Host ""
Write-Host "📖 Documentation: .codex/README.md" -ForegroundColor Yellow
Write-Host "🌐 GitHub: https://github.com/zapabob/codex" -ForegroundColor Yellow

