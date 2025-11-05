# MCP Server証明書生成スクリプト（Windows PowerShell版）
# 用途: TLS/mTLS用のCA証明書、Codex証明書、MCPサーバー証明書を生成
# 作成日: 2025-10-28
# 設計書: _docs/2025-10-28_セキュア通信アーキテクチャ設計書.md

$ErrorActionPreference = "Stop"

Write-Host "🔐 Codex MCP Server Certificate Generation Script (Windows)" -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host ""

# OpenSSLの確認
if (!(Get-Command openssl -ErrorAction SilentlyContinue)) {
    Write-Host "❌ OpenSSL not found. Please install OpenSSL first:" -ForegroundColor Red
    Write-Host "   - Download from: https://slproweb.com/products/Win32OpenSSL.html" -ForegroundColor Yellow
    Write-Host "   - Or use chocolatey: choco install openssl" -ForegroundColor Yellow
    exit 1
}

# ディレクトリ設定
$CertDir = "$env:USERPROFILE\.codex\certs"
$CADir = "$CertDir\ca"
$CodexDir = "$CertDir\codex"
$McpDir = "$CertDir\mcp-servers"
$KeysDir = "$env:USERPROFILE\.codex\keys"

# ディレクトリ作成
New-Item -ItemType Directory -Force -Path $CADir | Out-Null
New-Item -ItemType Directory -Force -Path $CodexDir | Out-Null
New-Item -ItemType Directory -Force -Path $McpDir | Out-Null
New-Item -ItemType Directory -Force -Path $KeysDir | Out-Null

Write-Host "📁 Directory structure created:" -ForegroundColor Green
Write-Host "   - CA: $CADir"
Write-Host "   - Codex: $CodexDir"
Write-Host "   - MCP Servers: $McpDir"
Write-Host "   - Keys: $KeysDir"
Write-Host ""

# ===========================================
# Step 1: CA証明書生成（有効期限10年）
# ===========================================
Write-Host "🏛️  Step 1: Generating CA Certificate (10 years)..." -ForegroundColor Yellow

$CACert = "$CADir\ca-cert.pem"
$CAKey = "$CADir\ca-key.pem"

if (Test-Path $CACert) {
    Write-Host "⚠️  CA certificate already exists. Skipping..." -ForegroundColor DarkYellow
} else {
    & openssl req -x509 -newkey rsa:4096 `
        -keyout $CAKey `
        -out $CACert `
        -days 3650 -nodes `
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=Security/CN=Codex CA" `
        2>$null
    
    Write-Host "✅ CA certificate generated: $CACert" -ForegroundColor Green
}
Write-Host ""

# ===========================================
# Step 2: Codex Core証明書生成（有効期限1年）
# ===========================================
Write-Host "🖥️  Step 2: Generating Codex Core Certificate (1 year)..." -ForegroundColor Yellow

$CodexCert = "$CodexDir\codex-cert.pem"
$CodexKey = "$CodexDir\codex-key.pem"
$CodexCSR = "$CodexDir\codex-csr.pem"

if (Test-Path $CodexCert) {
    Write-Host "⚠️  Codex certificate already exists. Skipping..." -ForegroundColor DarkYellow
} else {
    # CSR生成
    & openssl req -newkey rsa:4096 `
        -keyout $CodexKey `
        -out $CodexCSR `
        -nodes `
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=Core/CN=codex-core" `
        2>$null
    
    # CA署名
    & openssl x509 -req `
        -in $CodexCSR `
        -CA $CACert `
        -CAkey $CAKey `
        -CAcreateserial `
        -out $CodexCert `
        -days 365 `
        2>$null
    
    # CSR削除
    Remove-Item $CodexCSR -ErrorAction SilentlyContinue
    
    Write-Host "✅ Codex Core certificate generated: $CodexCert" -ForegroundColor Green
}
Write-Host ""

# ===========================================
# Step 3: MCPサーバー証明書生成（15サーバー）
# ===========================================
Write-Host "🔧 Step 3: Generating MCP Server Certificates (15 servers)..." -ForegroundColor Yellow

$McpServers = @(
    "codex",
    "serena",
    "gemini-cli",
    "context7",
    "playwright",
    "filesystem",
    "github",
    "youtube",
    "chrome-devtools",
    "sequential-thinking",
    "markitdown",
    "arxiv",
    "brave-search",
    "context7-2",
    "codex-gemini-mcp"
)

foreach ($server in $McpServers) {
    $ServerCert = "$McpDir\$server-cert.pem"
    $ServerKey = "$McpDir\$server-key.pem"
    $ServerCSR = "$McpDir\$server-csr.pem"
    
    if (Test-Path $ServerCert) {
        Write-Host "⚠️  Certificate for $server already exists. Skipping..." -ForegroundColor DarkYellow
        continue
    }
    
    Write-Host "   Generating certificate for $server..."
    
    # CSR生成
    & openssl req -newkey rsa:4096 `
        -keyout $ServerKey `
        -out $ServerCSR `
        -nodes `
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=MCP/CN=$server" `
        2>$null
    
    # CA署名
    & openssl x509 -req `
        -in $ServerCSR `
        -CA $CACert `
        -CAkey $CAKey `
        -CAcreateserial `
        -out $ServerCert `
        -days 365 `
        2>$null
    
    # CSR削除
    Remove-Item $ServerCSR -ErrorAction SilentlyContinue
    
    Write-Host "   ✅ $server certificate generated" -ForegroundColor Green
}
Write-Host ""

# ===========================================
# Step 4: Ed25519署名鍵生成（Codex Core用）
# ===========================================
Write-Host "🔑 Step 4: Generating Ed25519 Signing Keys..." -ForegroundColor Yellow

$SigningKey = "$KeysDir\ed25519-signing"

if (Test-Path $SigningKey) {
    Write-Host "⚠️  Ed25519 signing key already exists. Skipping..." -ForegroundColor DarkYellow
} else {
    # Windows用: ssh-keygen がない場合はスキップ
    if (Get-Command ssh-keygen -ErrorAction SilentlyContinue) {
        & ssh-keygen -t ed25519 `
            -f $SigningKey `
            -N '""' `
            -C "codex-core-signing" `
            2>$null | Out-Null
        
        Write-Host "✅ Ed25519 signing key generated: $SigningKey" -ForegroundColor Green
    } else {
        Write-Host "⚠️  ssh-keygen not found. Skipping Ed25519 key generation." -ForegroundColor DarkYellow
        Write-Host "   Install OpenSSH: Add-WindowsCapability -Online -Name OpenSSH.Client~~~~0.0.1.0" -ForegroundColor Yellow
    }
}
Write-Host ""

# ===========================================
# Step 5: Agent署名鍵生成（8エージェント）
# ===========================================
Write-Host "🤖 Step 5: Generating Agent Ed25519 Keys (8 agents)..." -ForegroundColor Yellow

$Agents = @(
    "codeexpert",
    "securityexpert",
    "testingexpert",
    "docsexpert",
    "deepresearcher",
    "debugexpert",
    "performanceexpert",
    "general"
)

$AgentsDir = "$KeysDir\agents"
New-Item -ItemType Directory -Force -Path $AgentsDir | Out-Null

if (Get-Command ssh-keygen -ErrorAction SilentlyContinue) {
    foreach ($agent in $Agents) {
        $AgentKey = "$AgentsDir\$agent-signing"
        
        if (Test-Path $AgentKey) {
            Write-Host "⚠️  Key for $agent already exists. Skipping..." -ForegroundColor DarkYellow
            continue
        }
        
        Write-Host "   Generating Ed25519 keypair for $agent..."
        & ssh-keygen -t ed25519 `
            -f $AgentKey `
            -N '""' `
            -C "codex-agent-$agent" `
            2>$null | Out-Null
        
        Write-Host "   ✅ $agent keypair generated" -ForegroundColor Green
    }
} else {
    Write-Host "⚠️  ssh-keygen not found. Skipping agent key generation." -ForegroundColor DarkYellow
}
Write-Host ""

# ===========================================
# Step 6: パーミッション設定（Windowsでは制限的）
# ===========================================
Write-Host "🔒 Step 6: Setting secure file permissions..." -ForegroundColor Yellow
Write-Host "   ⚠️  Windows does not support Unix-style permissions (400, 600)." -ForegroundColor DarkYellow
Write-Host "   ℹ️  Ensure .codex directory is protected by Windows ACLs." -ForegroundColor Cyan
Write-Host ""

# ===========================================
# Step 7: 証明書情報表示
# ===========================================
Write-Host "📋 Step 7: Certificate Information" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan

Write-Host "🏛️  CA Certificate:" -ForegroundColor Green
& openssl x509 -in $CACert -noout -subject -issuer -dates 2>$null | ForEach-Object { "   $_" }

Write-Host ""
Write-Host "🖥️  Codex Core Certificate:" -ForegroundColor Green
& openssl x509 -in $CodexCert -noout -subject -issuer -dates 2>$null | ForEach-Object { "   $_" }

Write-Host ""
Write-Host "🔧 MCP Server Certificates:" -ForegroundColor Green
foreach ($server in $McpServers) {
    $ServerCert = "$McpDir\$server-cert.pem"
    if (Test-Path $ServerCert) {
        Write-Host "   - $server:" -ForegroundColor Yellow
        & openssl x509 -in $ServerCert -noout -subject -dates 2>$null | ForEach-Object { "     $_" }
    }
}

Write-Host ""
Write-Host "🔑 Signing Keys:" -ForegroundColor Green
Write-Host "   - Codex Core: $SigningKey"
Write-Host "   - Agents (8): $AgentsDir\*-signing"

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "✅ Certificate Generation Complete!" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "⚠️  IMPORTANT SECURITY NOTES:" -ForegroundColor Red
Write-Host "   1. Backup $CAKey to a secure location (offline)" -ForegroundColor Yellow
Write-Host "   2. Never commit private keys (*.pem, *-key.pem, *-signing) to Git" -ForegroundColor Yellow
Write-Host "   3. Rotate certificates before expiration (365 days)" -ForegroundColor Yellow
Write-Host "   4. Use strong passwords for production environments" -ForegroundColor Yellow
Write-Host ""
Write-Host "📚 Next Steps:" -ForegroundColor Cyan
Write-Host "   1. Update config.toml with certificate paths" -ForegroundColor White
Write-Host "   2. Enable TLS/mTLS in [security] section" -ForegroundColor White
Write-Host "   3. Restart Codex services" -ForegroundColor White
Write-Host "   4. Verify connections: codex mcp list" -ForegroundColor White
Write-Host ""
Write-Host "📖 Documentation: _docs\2025-10-28_セキュア通信アーキテクチャ設計書.md" -ForegroundColor Cyan
Write-Host ""

