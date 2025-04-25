#!/usr/bin/env bash
set -euo pipefail

# Detect Homebrew’s prefix
BREW_BIN="$(command -v brew || true)"
if [[ -z "$BREW_BIN" ]]; then
  echo "Homebrew not found. Please install Homebrew first."
  exit 1
fi

# Helper to run Homebrew under ARM64
hb() {
  arch -arm64 brew "$@"
}

echo "Using Homebrew at: $BREW_BIN"

# 1. Install Go
if ! command -v go &>/dev/null; then
  echo "Installing Go…"
  hb install go
fi

# 2. Install nmap
if ! command -v nmap &>/dev/null; then
  echo "Installing nmap…"
  hb install nmap
fi

# 3. Install whatweb
if ! command -v whatweb &>/dev/null; then
  echo "Installing whatweb…"
  hb install whatweb
fi

# 4. Install dirsearch (Python)
if ! command -v dirsearch &>/dev/null; then
  echo "Installing dirsearch…"
  pip3 install --upgrade dirsearch
fi

# 5. Install ffuf (Go)
if ! command -v ffuf &>/dev/null; then
  echo "Installing ffuf via Go…"
  GOBIN="$(brew --prefix)/bin" go install github.com/ffuf/ffuf@latest
fi

# 6. Install nuclei (Homebrew tap)
if ! command -v nuclei &>/dev/null; then
  echo "Tapping and installing nuclei…"
  hb tap projectdiscovery/tap
  hb install projectdiscovery/tap/nuclei
fi

# 7. Install sqlmap (Python)
if ! command -v sqlmap &>/dev/null; then
  echo "Installing sqlmap…"
  pip3 install sqlmap
fi

# 8. Install dalfox (Go)
if ! command -v dalfox &>/dev/null; then
  echo "Installing dalfox via Go…"
  GOBIN="$(brew --prefix)/bin" go install github.com/hahwul/dalfox@latest
fi

# 9. Create session-extractor helper script
SESSION_EXTRACTOR_PATH="$(brew --prefix)/bin/session-extractor"
echo "Creating session-extractor at $SESSION_EXTRACTOR_PATH…"
cat << 'EOF' > "$SESSION_EXTRACTOR_PATH"
#!/usr/bin/env bash
# Usage: session-extractor <target>
grep -Eo 'session=[a-zA-Z0-9]+' /tmp/sqlmap/output/"$1"/log
EOF
chmod +x "$SESSION_EXTRACTOR_PATH"

echo "✅ All 8 tools have been installed and session-extractor created."

