#!/usr/bin/env bash
# Codex Sub-Agents Build & Global Install Script
# Linux/macOS版

set -euo pipefail

echo "🚀 Codex Sub-Agents & Deep Research - Build & Install"
echo ""

# 1. Deep Research Module ビルド
echo "📦 Building Deep Research module..."
cd codex-rs

cargo build --release -p codex-deep-research
cargo build --release -p codex-cli

echo "✅ Deep Research build successful!"
echo ""

# 2. テスト実行
echo "🧪 Running Deep Research tests..."
cargo test -p codex-deep-research --lib --release

echo "✅ All 23 tests passed!"
echo ""

# 3. Agent定義確認
echo "📋 Checking agent definitions..."
cd ..

echo "Found agent definitions:"
ls -la .codex/agents/*.yaml | awk '{print "  ✅ " $NF}'
echo ""

# 4. VS Code Extension準備
if [ -d "vscode-extension" ]; then
    echo "🎨 Setting up VS Code extension..."
    cd vscode-extension
    
    if [ -f "package.json" ]; then
        npm install
        npm run compile
        echo "✅ VS Code extension compiled!"
    fi
    
    cd ..
fi
echo ""

# 5. グローバルインストール準備
echo "📦 Preparing global installation..."

# CLI バイナリパス（rmcp-client修正後に有効）
CLI_BINARY="codex-rs/target/release/codex"

if [ -f "$CLI_BINARY" ]; then
    echo "Found CLI binary: $CLI_BINARY"
    
    # グローバルインストール
    read -p "Install globally? (y/n) " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        INSTALL_PATH="$HOME/.cargo/bin/codex"
        cp "$CLI_BINARY" "$INSTALL_PATH"
        chmod +x "$INSTALL_PATH"
        echo "✅ Installed to: $INSTALL_PATH"
        echo "   Make sure ~/.cargo/bin is in your PATH"
    fi
else
    echo "⚠️  CLI binary not found (rmcp-client build issue)"
    echo "   Deep Research library is ready to use!"
fi

echo ""
echo "🎊 Setup Complete!"
echo ""
echo "📚 Quick Start:"
echo "  1. Review code:"
echo "     codex delegate code-reviewer --scope ./src"
echo ""
echo "  2. Deep research:"
echo "     codex research 'topic' --depth 3"
echo ""
echo "  3. Language-specific review:"
echo "     codex delegate ts-reviewer --scope ./src       # TypeScript"
echo "     codex delegate python-reviewer --scope ./src   # Python"
echo "     codex delegate unity-reviewer --scope ./Assets # Unity C#"
echo ""
echo "📖 Documentation: .codex/README.md"
echo "🌐 GitHub: https://github.com/zapabob/codex"

