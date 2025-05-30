#!/bin/bash

# Mixture-of-Idiots Startup Script
# Launches all three components in separate terminal windows

echo "🚀 Starting Mixture-of-Idiots System..."

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo "❌ ERROR: .env file not found!"
    echo "Please create a .env file with your OpenAI API key:"
    echo "echo 'OPENAI_API_KEY=your_key_here' > .env"
    exit 1
fi

# Get the current directory
BRIDGE_DIR="$(pwd)"

echo "📂 Bridge Directory: $BRIDGE_DIR"
echo "🔑 Loading API key from .env file..."

# Make scripts executable
chmod +x smart_bridge.js
chmod +x master_control.js
chmod +x claude_enhanced.js
chmod +x codex_enhanced.js

echo "✅ Scripts made executable"

# Function to launch in new terminal windows
launch_terminal() {
    local title="$1"
    local command="$2"
    local color="$3"
    
    echo "🪟 Launching $title..."
    
    # For WSL, we'll use different approaches depending on what's available
    if command -v wt >/dev/null 2>&1; then
        # Windows Terminal
        wt.exe new-tab --title "$title" bash -c "cd '$BRIDGE_DIR' && echo -e '$color$title Started' && $command; read -p 'Press Enter to close...'"
    elif command -v gnome-terminal >/dev/null 2>&1; then
        # GNOME Terminal
        gnome-terminal --title="$title" --tab -- bash -c "cd '$BRIDGE_DIR' && echo -e '$color$title Started' && $command; read -p 'Press Enter to close...'"
    elif command -v xterm >/dev/null 2>&1; then
        # XTerm
        xterm -title "$title" -e "cd '$BRIDGE_DIR' && echo -e '$color$title Started' && $command; read -p 'Press Enter to close...'" &
    else
        # Fallback - run in background with tmux if available
        if command -v tmux >/dev/null 2>&1; then
            echo "Using tmux session for $title"
            tmux new-session -d -s "mixture-$title" "cd '$BRIDGE_DIR' && $command"
        else
            echo "⚠️  No terminal emulator found. Please run manually:"
            echo "   Terminal: $title"
            echo "   Command: cd '$BRIDGE_DIR' && $command"
            return 1
        fi
    fi
    
    sleep 1
}

echo ""
echo "🌟 Launching Mixture-of-Idiots Components..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Launch Smart Bridge (Terminal 1)
launch_terminal "Smart Bridge" "node smart_bridge.js" "\033[1;36m"

# Wait a moment for bridge to initialize
sleep 2

# Launch Codex Enhanced (Terminal 2)
launch_terminal "Codex Enhanced" "node codex_enhanced.js" "\033[1;33m"

# Wait a moment for Codex to initialize
sleep 2

# Launch Claude Enhanced (Terminal 3)
launch_terminal "Claude Enhanced" "node claude_enhanced.js" "\033[1;34m"

# Wait a moment for Claude to initialize
sleep 2

# Launch Master Control (Terminal 4) - This one the user will interact with
launch_terminal "Master Control" "node master_control.js" "\033[1;32m"

echo ""
echo "🎉 MIXTURE-OF-IDIOTS SYSTEM LAUNCHED!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📱 Four terminals should now be open:"
echo "   🌉 Smart Bridge    - Routes messages intelligently"
echo "   🟠 Codex Enhanced  - Connects to Codex CLI"
echo "   🔵 Claude Enhanced - Connects to Claude Code"
echo "   🎮 Master Control  - Your control interface"
echo ""
echo "💡 In Master Control terminal, try these commands:"
echo "   /claude <message>  - Send message directly to Claude"
echo "   /codex <message>   - Send message directly to Codex"
echo "   <regular message>  - Continue AI-to-AI conversation"
echo "   /help             - Show all commands"
echo ""
echo "🤖 The AIs will start talking to each other automatically!"
echo "🎯 You can intervene anytime with /claude or /codex commands"
echo ""

# If tmux was used, show session info
if command -v tmux >/dev/null 2>&1 && tmux list-sessions | grep -q mixture; then
    echo "📺 Tmux sessions created. To view:"
    echo "   tmux attach -t mixture-Smart-Bridge"
    echo "   tmux attach -t mixture-Codex-Enhanced"
    echo "   tmux attach -t mixture-Claude-Enhanced"
    echo "   tmux attach -t mixture-Master-Control"
    echo ""
fi

echo "🔥 Ready for AI collaboration!"