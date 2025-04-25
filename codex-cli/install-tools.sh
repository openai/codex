#!/usr/bin/env bash
set -euo pipefail

# 1. Ensure Homebrew is installed
if ! command -v brew &>/dev/null; then
  echo "Homebrew not found. Installing Homebrew…"
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
  eval "$(/opt/homebrew/bin/brew shellenv)"
fi

# 2. Install Go (required for ffuf and dalfox)
if ! command -v go &>/dev/null; then
  echo "Installing Go…"
  brew install go
fi

# 3. Install nmap
if ! command -v nmap &>/dev/null; then
  echo "Installing nmap…"
  brew install nmap
fi

# 4. Install whatweb
if ! command -v whatweb &>/dev/null; then
  echo "Installing whatweb…"
  brew install whatweb
fi

# 5. Install dirsearch (Python)
if ! command -v dirsearch &>/dev/null; then
  echo "Installing dirsearch…"
  pip3 install --upgrade dirsearch
fi

# 6. Install ffuf (Go)
if ! command -v ffuf &>/dev/null; then
  echo "Installing ffuf via Go…"
  GOBIN="$(brew --prefix)/bin" go install github.com/ffuf/ffuf@latest
fi

# 7. Install nuclei (Homebrew tap)
if ! command -v nuclei &>/dev/null; then
  echo "Tapping and installing nuclei…"
  brew tap projectdiscovery/tap
  brew install projectdiscovery/tap/nuclei
fi

# 8. Install sqlmap (Python)
if ! command -v sqlmap &>/dev/null; then
  echo "Installing sqlmap…"
  pip3 install sqlmap
fi

# 9. Install dalfox (Go)
if ! command -v dalfox &>/dev/null; then
  echo "Installing dalfox via Go…"
  GOBIN="$(brew --prefix)/bin" go install github.com/hahwul/dalfox@latest
fi

# 10. Create session-extractor helper script
SESSION_EXTRACTOR_PATH="$(brew --prefix)/bin/session-extractor"
echo "Creating session-extractor at $SESSION_EXTRACTOR_PATH…"
cat << 'EOF' > "$SESSION_EXTRACTOR_PATH"
#!/usr/bin/env bash
# Usage: session-extractor <target>
grep -Eo 'session=[a-zA-Z0-9]+' /tmp/sqlmap/output/"$1"/log
EOF
chmod +x "$SESSION_EXTRACTOR_PATH"

echo "✅ All 8 tools have been installed and session-extractor created."

