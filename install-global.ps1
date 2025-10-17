# Codex グローバルインストールスクリプト
# Windows PowerShell版 - 本番環境対応

Write-Host "🚀 Codex Global Installation - Production Ready" -ForegroundColor Cyan
Write-Host ""

# 管理者権限チェック
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "⚠️  Administrator privileges recommended for global installation" -ForegroundColor Yellow
}

# 1. Rust バイナリビルド（既にビルド済みの場合はスキップ）
Write-Host "📦 Checking Rust binaries..." -ForegroundColor Yellow
$binariesExist = (Test-Path "codex-rs\target\release\codex-tui.exe") -and (Test-Path "codex-rs\target\release\codex.exe")

if (-not $binariesExist) {
    Write-Host "Building Rust binaries..." -ForegroundColor Yellow
    Set-Location -Path "codex-rs"
    cargo build --release --bin codex --bin codex-tui
    Set-Location -Path ".."
}

# 2. バイナリ確認
Write-Host ""
Write-Host "✅ Available binaries:" -ForegroundColor Green
Get-ChildItem -Path "codex-rs\target\release\*.exe" | ForEach-Object {
    Write-Host "   - $($_.Name)" -ForegroundColor Green
}

# 3. グローバルインストール先
$installDir = "$env:USERPROFILE\.codex\bin"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Write-Host ""
    Write-Host "📁 Created installation directory: $installDir" -ForegroundColor Yellow
}

# 4. バイナリコピー
Write-Host ""
Write-Host "📦 Installing binaries to $installDir..." -ForegroundColor Yellow

$binaries = @(
    "codex.exe",
    "codex-tui.exe",
    "codex-mcp-server.exe",
    "codex-mcp-client.exe"
)

foreach ($binary in $binaries) {
    $srcPath = "codex-rs\target\release\$binary"
    if (Test-Path $srcPath) {
        Copy-Item -Path $srcPath -Destination $installDir -Force
        Write-Host "   ✅ Installed: $binary" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  Not found: $binary (skipping)" -ForegroundColor Yellow
    }
}

# 5. MCP サーバースクリプト
Write-Host ""
Write-Host "📦 Installing MCP server scripts..." -ForegroundColor Yellow
$mcpScripts = @(
    "codex-rs\mcp-server\dist\index.js",
    "codex-rs\deep-research\mcp-server\web-search.js"
)

foreach ($script in $mcpScripts) {
    if (Test-Path $script) {
        $destName = Split-Path -Leaf $script
        Copy-Item -Path $script -Destination "$installDir\$destName" -Force
        Write-Host "   ✅ Installed: $destName" -ForegroundColor Green
    }
}

# 6. エージェント定義コピー
Write-Host ""
Write-Host "📦 Installing agent definitions..." -ForegroundColor Yellow
$agentsDir = "$env:USERPROFILE\.codex\agents"
if (-not (Test-Path $agentsDir)) {
    New-Item -ItemType Directory -Force -Path $agentsDir | Out-Null
}

Copy-Item -Path ".codex\agents\*" -Destination $agentsDir -Force -Recurse
$agentCount = (Get-ChildItem -Path $agentsDir -Filter "*.yaml").Count
Write-Host "   ✅ Installed $agentCount agent definitions" -ForegroundColor Green

# 7. PATH設定確認
Write-Host ""
Write-Host "🔧 Checking PATH configuration..." -ForegroundColor Yellow

$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$installDir*") {
    Write-Host "   ⚠️  $installDir is not in your PATH" -ForegroundColor Yellow
    Write-Host ""
    $addToPath = Read-Host "   Add to PATH? (y/n)"
    
    if ($addToPath -eq "y") {
        $newPath = "$currentPath;$installDir"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "   ✅ Added to PATH (restart terminal to apply)" -ForegroundColor Green
    }
} else {
    Write-Host "   ✅ Already in PATH" -ForegroundColor Green
}

# 8. 環境変数テンプレート
Write-Host ""
Write-Host "📝 Creating .env template..." -ForegroundColor Yellow
$envTemplate = @"
# Codex Environment Variables
# Copy to .env and fill in your API keys

# Web Search API Keys (for Deep Research)
BRAVE_API_KEY=your_brave_api_key_here
GOOGLE_API_KEY=your_google_api_key_here
GOOGLE_CSE_ID=your_google_cse_id_here
BING_API_KEY=your_bing_api_key_here

# OpenAI (optional)
OPENAI_API_KEY=your_openai_api_key_here

# MCP Server (optional)
MCP_SERVER_URL=http://localhost:3000
"@

$envPath = "$env:USERPROFILE\.codex\.env.template"
Set-Content -Path $envPath -Value $envTemplate
Write-Host "   ✅ Template saved: $envPath" -ForegroundColor Green

# 9. 動作確認
Write-Host ""
Write-Host "🧪 Testing installation..." -ForegroundColor Yellow

# MCPサーバーテスト
if (Test-Path "codex-rs\mcp-server\test\test-server.js") {
    Write-Host "   Running MCP server tests..." -ForegroundColor Cyan
    $testResult = node codex-rs\mcp-server\test\test-server.js 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "   ✅ MCP server tests passed" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  MCP server tests had issues (non-critical)" -ForegroundColor Yellow
    }
}

# 10. インストール完了サマリー
Write-Host ""
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "   🎊 Installation Complete!" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

Write-Host "📍 Installation Directory:" -ForegroundColor Yellow
Write-Host "   $installDir" -ForegroundColor White
Write-Host ""

Write-Host "📚 Quick Start:" -ForegroundColor Yellow
Write-Host "   1. Configure API keys:" -ForegroundColor White
Write-Host "      Copy $env:USERPROFILE\.codex\.env.template to .env" -ForegroundColor Gray
Write-Host ""
Write-Host "   2. Start MCP server:" -ForegroundColor White
Write-Host "      node `"$installDir\index.js`"" -ForegroundColor Gray
Write-Host ""
Write-Host "   3. Run Deep Research:" -ForegroundColor White
Write-Host "      codex-tui research `"topic`" --depth 3" -ForegroundColor Gray
Write-Host ""
Write-Host "   4. Code Review:" -ForegroundColor White
Write-Host "      codex-tui delegate code-reviewer --scope ./src" -ForegroundColor Gray
Write-Host ""

Write-Host "📖 Documentation:" -ForegroundColor Yellow
Write-Host "   - Setup Guide: CURSOR_IDE_SETUP.md" -ForegroundColor Gray
Write-Host "   - Agent Docs: .codex/README.md" -ForegroundColor Gray
Write-Host "   - Install Docs: INSTALL_SUBAGENTS.md" -ForegroundColor Gray
Write-Host ""

Write-Host "🌐 GitHub: https://github.com/zapabob/codex" -ForegroundColor Yellow
Write-Host ""

Write-Host "✨ Ready to use Codex Multi-Agent System!" -ForegroundColor Green

