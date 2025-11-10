# Codex VSIX パッケージングスクリプト
# Cursor統合用VSIXファイルを自動生成するで〜

param(
    [string]$Version = "",
    [switch]$Clean = $false,
    [switch]$Install = $false
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# カラー出力用
function Write-ColorOutput($ForegroundColor, $Message) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    Write-Output $Message
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Progress-Bar {
    param(
        [int]$Percent,
        [string]$Activity,
        [string]$Status
    )
    Write-Progress -Activity $Activity -Status $Status -PercentComplete $Percent
}

# プロジェクトルート取得
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$extensionRoot = $scriptRoot
$projectRoot = Split-Path -Parent (Split-Path -Parent $scriptRoot)

Write-ColorOutput Green "🚀 Codex VSIX パッケージング開始やで〜"
Write-ColorOutput Cyan "📁 Extension Root: $extensionRoot"
Write-ColorOutput Cyan "📁 Project Root: $projectRoot"

# バージョン取得
if ([string]::IsNullOrEmpty($Version)) {
    $packageJson = Get-Content "$extensionRoot/package.json" | ConvertFrom-Json
    $Version = $packageJson.version
}
Write-ColorOutput Yellow "📦 Version: $Version"

# クリーンビルド
if ($Clean) {
    Write-ColorOutput Yellow "🧹 クリーンビルド実行中..."
    Write-Progress-Bar -Percent 10 -Activity "Cleaning" -Status "Removing old files"
    
    Remove-Item -Path "$extensionRoot/out" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$extensionRoot/*.vsix" -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$extensionRoot/node_modules/.cache" -Recurse -Force -ErrorAction SilentlyContinue
    
    Write-ColorOutput Green "✅ クリーン完了"
}

# 依存関係インストール
Write-ColorOutput Yellow "📥 依存関係インストール中..."
Write-Progress-Bar -Percent 20 -Activity "Installing" -Status "Installing npm dependencies"

Push-Location $extensionRoot
try {
    if (-not (Test-Path "node_modules")) {
        npm install
        if ($LASTEXITCODE -ne 0) {
            throw "npm install failed"
        }
    }
    Write-ColorOutput Green "✅ 依存関係インストール完了"
} finally {
    Pop-Location
}

# TypeScriptコンパイル
Write-ColorOutput Yellow "🔨 TypeScriptコンパイル中..."
Write-Progress-Bar -Percent 40 -Activity "Compiling" -Status "Compiling TypeScript"

Push-Location $extensionRoot
try {
    npm run compile
    if ($LASTEXITCODE -ne 0) {
        throw "TypeScript compilation failed"
    }
    Write-ColorOutput Green "✅ コンパイル完了"
} finally {
    Pop-Location
}

# vsceパッケージング
Write-ColorOutput Yellow "📦 VSIXパッケージング中..."
Write-Progress-Bar -Percent 70 -Activity "Packaging" -Status "Creating VSIX file"

Push-Location $extensionRoot
try {
    # vsceがインストールされているか確認
    $vsceInstalled = npm list -g @vscode/vsce 2>$null
    if (-not $vsceInstalled) {
        Write-ColorOutput Yellow "📦 vsceをグローバルインストール中..."
        npm install -g @vscode/vsce
    }
    
    # VSIXパッケージ作成
    $vsixFileName = "codex-assistant-$Version.vsix"
    vsce package --out $vsixFileName
    
    if ($LASTEXITCODE -ne 0) {
        throw "VSIX packaging failed"
    }
    
    $vsixPath = Join-Path $extensionRoot $vsixFileName
    if (Test-Path $vsixPath) {
        $fileSize = (Get-Item $vsixPath).Length / 1MB
        Write-ColorOutput Green "✅ VSIXパッケージ作成完了: $vsixFileName ($([math]::Round($fileSize, 2)) MB)"
    } else {
        throw "VSIX file not found after packaging"
    }
} finally {
    Pop-Location
}

# Cursor統合用MCP設定ファイル生成
Write-ColorOutput Yellow "🔗 Cursor統合用MCP設定ファイル生成中..."
Write-Progress-Bar -Percent 85 -Activity "Configuring" -Status "Generating MCP config"

$mcpConfigPath = Join-Path $projectRoot ".cursor/mcp.json"
$mcpConfigDir = Split-Path -Parent $mcpConfigPath

if (-not (Test-Path $mcpConfigDir)) {
    New-Item -ItemType Directory -Path $mcpConfigDir -Force | Out-Null
}

$mcpConfig = @{
    mcpServers = @{
        codex = @{
            command = "codex"
            args = @("mcp-server")
            env = @{}
            description = "Codex Multi-Agent System with Deep Research, Sub-Agents, and Blueprint Mode"
            disabled = $false
        }
    }
} | ConvertTo-Json -Depth 10

Set-Content -Path $mcpConfigPath -Value $mcpConfig -Encoding UTF8
Write-ColorOutput Green "✅ MCP設定ファイル生成完了: $mcpConfigPath"

# インストール（オプション）
if ($Install) {
    Write-ColorOutput Yellow "📥 VSIXインストール中..."
    Write-Progress-Bar -Percent 95 -Activity "Installing" -Status "Installing VSIX to Cursor"
    
    $cursorPath = "$env:LOCALAPPDATA\Programs\cursor\Cursor.exe"
    if (Test-Path $cursorPath) {
        & $cursorPath --install-extension $vsixPath
        Write-ColorOutput Green "✅ VSIXインストール完了（Cursor再起動が必要やで）"
    } else {
        Write-ColorOutput Yellow "⚠️  Cursorが見つからんかった。手動でインストールしてくれ:"
        Write-ColorOutput Cyan "   code --install-extension $vsixPath"
    }
}

Write-Progress-Bar -Percent 100 -Activity "Complete" -Status "Done"
Write-ColorOutput Green "🎉 パッケージング完了やで〜！"
Write-ColorOutput Cyan "📦 VSIXファイル: $vsixPath"
Write-ColorOutput Cyan "🔗 MCP設定: $mcpConfigPath"

