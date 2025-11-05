#!/bin/bash
# MCP Server証明書生成スクリプト
# 用途: TLS/mTLS用のCA証明書、Codex証明書、MCPサーバー証明書を生成
# 作成日: 2025-10-28
# 設計書: _docs/2025-10-28_セキュア通信アーキテクチャ設計書.md

set -euo pipefail

echo "🔐 Codex MCP Server Certificate Generation Script"
echo "=================================================="

# ディレクトリ設定
CERT_DIR="$HOME/.codex/certs"
CA_DIR="$CERT_DIR/ca"
CODEX_DIR="$CERT_DIR/codex"
MCP_DIR="$CERT_DIR/mcp-servers"
KEYS_DIR="$HOME/.codex/keys"

# ディレクトリ作成
mkdir -p "$CA_DIR" "$CODEX_DIR" "$MCP_DIR" "$KEYS_DIR"

echo "📁 Directory structure created:"
echo "   - CA: $CA_DIR"
echo "   - Codex: $CODEX_DIR"
echo "   - MCP Servers: $MCP_DIR"
echo "   - Keys: $KEYS_DIR"
echo ""

# ===========================================
# Step 1: CA証明書生成（有効期限10年）
# ===========================================
echo "🏛️  Step 1: Generating CA Certificate (10 years)..."

if [ -f "$CA_DIR/ca-cert.pem" ]; then
    echo "⚠️  CA certificate already exists. Skipping..."
else
    openssl req -x509 -newkey rsa:4096 \
        -keyout "$CA_DIR/ca-key.pem" \
        -out "$CA_DIR/ca-cert.pem" \
        -days 3650 -nodes \
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=Security/CN=Codex CA" \
        2>/dev/null
    
    echo "✅ CA certificate generated: $CA_DIR/ca-cert.pem"
fi
echo ""

# ===========================================
# Step 2: Codex Core証明書生成（有効期限1年）
# ===========================================
echo "🖥️  Step 2: Generating Codex Core Certificate (1 year)..."

if [ -f "$CODEX_DIR/codex-cert.pem" ]; then
    echo "⚠️  Codex certificate already exists. Skipping..."
else
    # CSR生成
    openssl req -newkey rsa:4096 \
        -keyout "$CODEX_DIR/codex-key.pem" \
        -out "$CODEX_DIR/codex-csr.pem" \
        -nodes \
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=Core/CN=codex-core" \
        2>/dev/null
    
    # CA署名
    openssl x509 -req \
        -in "$CODEX_DIR/codex-csr.pem" \
        -CA "$CA_DIR/ca-cert.pem" \
        -CAkey "$CA_DIR/ca-key.pem" \
        -CAcreateserial \
        -out "$CODEX_DIR/codex-cert.pem" \
        -days 365 \
        2>/dev/null
    
    # CSR削除
    rm "$CODEX_DIR/codex-csr.pem"
    
    echo "✅ Codex Core certificate generated: $CODEX_DIR/codex-cert.pem"
fi
echo ""

# ===========================================
# Step 3: MCPサーバー証明書生成（15サーバー）
# ===========================================
echo "🔧 Step 3: Generating MCP Server Certificates (15 servers)..."

MCP_SERVERS=(
    "codex"
    "serena"
    "gemini-cli"
    "context7"
    "playwright"
    "filesystem"
    "github"
    "youtube"
    "chrome-devtools"
    "sequential-thinking"
    "markitdown"
    "arxiv"
    "brave-search"
    "context7-2"
    "codex-gemini-mcp"
)

for server in "${MCP_SERVERS[@]}"; do
    if [ -f "$MCP_DIR/${server}-cert.pem" ]; then
        echo "⚠️  Certificate for $server already exists. Skipping..."
        continue
    fi
    
    echo "   Generating certificate for $server..."
    
    # CSR生成
    openssl req -newkey rsa:4096 \
        -keyout "$MCP_DIR/${server}-key.pem" \
        -out "$MCP_DIR/${server}-csr.pem" \
        -nodes \
        -subj "/C=JP/ST=Tokyo/L=Tokyo/O=Codex/OU=MCP/CN=${server}" \
        2>/dev/null
    
    # CA署名
    openssl x509 -req \
        -in "$MCP_DIR/${server}-csr.pem" \
        -CA "$CA_DIR/ca-cert.pem" \
        -CAkey "$CA_DIR/ca-key.pem" \
        -CAcreateserial \
        -out "$MCP_DIR/${server}-cert.pem" \
        -days 365 \
        2>/dev/null
    
    # CSR削除
    rm "$MCP_DIR/${server}-csr.pem"
    
    echo "   ✅ $server certificate generated"
done
echo ""

# ===========================================
# Step 4: Ed25519署名鍵生成（Codex Core用）
# ===========================================
echo "🔑 Step 4: Generating Ed25519 Signing Keys..."

if [ -f "$KEYS_DIR/ed25519-signing" ]; then
    echo "⚠️  Ed25519 signing key already exists. Skipping..."
else
    ssh-keygen -t ed25519 \
        -f "$KEYS_DIR/ed25519-signing" \
        -N "" \
        -C "codex-core-signing" \
        >/dev/null 2>&1
    
    echo "✅ Ed25519 signing key generated: $KEYS_DIR/ed25519-signing"
fi
echo ""

# ===========================================
# Step 5: Agent署名鍵生成（8エージェント）
# ===========================================
echo "🤖 Step 5: Generating Agent Ed25519 Keys (8 agents)..."

AGENTS=(
    "codeexpert"
    "securityexpert"
    "testingexpert"
    "docsexpert"
    "deepresearcher"
    "debugexpert"
    "performanceexpert"
    "general"
)

AGENTS_DIR="$KEYS_DIR/agents"
mkdir -p "$AGENTS_DIR"

for agent in "${AGENTS[@]}"; do
    if [ -f "$AGENTS_DIR/${agent}-signing" ]; then
        echo "⚠️  Key for $agent already exists. Skipping..."
        continue
    fi
    
    echo "   Generating Ed25519 keypair for $agent..."
    ssh-keygen -t ed25519 \
        -f "$AGENTS_DIR/${agent}-signing" \
        -N "" \
        -C "codex-agent-${agent}" \
        >/dev/null 2>&1
    
    echo "   ✅ $agent keypair generated"
done
echo ""

# ===========================================
# Step 6: パーミッション設定
# ===========================================
echo "🔒 Step 6: Setting secure file permissions..."

# CA秘密鍵（最高レベルの保護）
chmod 400 "$CA_DIR/ca-key.pem"
echo "   🔴 CA private key: 400 (read-only for owner)"

# Codex秘密鍵
chmod 600 "$CODEX_DIR/codex-key.pem"
echo "   🟡 Codex private key: 600 (read/write for owner)"

# MCPサーバー秘密鍵
chmod 600 "$MCP_DIR"/*-key.pem 2>/dev/null || true
echo "   🟡 MCP server private keys: 600"

# 署名鍵
chmod 600 "$KEYS_DIR/ed25519-signing" 2>/dev/null || true
chmod 600 "$AGENTS_DIR"/*-signing 2>/dev/null || true
echo "   🟡 Signing keys: 600"

# 証明書（公開鍵）は読み取り可能
chmod 644 "$CA_DIR/ca-cert.pem" 2>/dev/null || true
chmod 644 "$CODEX_DIR/codex-cert.pem" 2>/dev/null || true
chmod 644 "$MCP_DIR"/*-cert.pem 2>/dev/null || true
chmod 644 "$KEYS_DIR/ed25519-signing.pub" 2>/dev/null || true
chmod 644 "$AGENTS_DIR"/*-signing.pub 2>/dev/null || true
echo "   🟢 Public certificates/keys: 644 (readable)"
echo ""

# ===========================================
# Step 7: 証明書情報表示
# ===========================================
echo "📋 Step 7: Certificate Information"
echo "====================================="

echo "🏛️  CA Certificate:"
openssl x509 -in "$CA_DIR/ca-cert.pem" -noout -subject -issuer -dates 2>/dev/null | sed 's/^/   /'

echo ""
echo "🖥️  Codex Core Certificate:"
openssl x509 -in "$CODEX_DIR/codex-cert.pem" -noout -subject -issuer -dates 2>/dev/null | sed 's/^/   /'

echo ""
echo "🔧 MCP Server Certificates:"
for server in "${MCP_SERVERS[@]}"; do
    if [ -f "$MCP_DIR/${server}-cert.pem" ]; then
        echo "   - $server:"
        openssl x509 -in "$MCP_DIR/${server}-cert.pem" -noout -subject -dates 2>/dev/null | sed 's/^/     /'
    fi
done

echo ""
echo "🔑 Signing Keys:"
echo "   - Codex Core: $KEYS_DIR/ed25519-signing"
echo "   - Agents (8): $AGENTS_DIR/*-signing"

echo ""
echo "====================================="
echo "✅ Certificate Generation Complete!"
echo "====================================="
echo ""
echo "⚠️  IMPORTANT SECURITY NOTES:"
echo "   1. Backup $CA_DIR/ca-key.pem to a secure location (offline)"
echo "   2. Never commit private keys (*.pem, *-key.pem, *-signing) to Git"
echo "   3. Rotate certificates before expiration (365 days)"
echo "   4. Use strong passwords for production environments"
echo ""
echo "📚 Next Steps:"
echo "   1. Update config.toml with certificate paths"
echo "   2. Enable TLS/mTLS in [security] section"
echo "   3. Restart Codex services"
echo "   4. Verify connections: codex mcp list"
echo ""
echo "📖 Documentation: _docs/2025-10-28_セキュア通信アーキテクチャ設計書.md"
echo ""

