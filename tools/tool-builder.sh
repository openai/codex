#!/usr/bin/env bash
set -euo pipefail

# Tool Builder: Scaffold, configure, and launch a new CLI project using Codex CLI, ChatGPT, GitHub, and Codespaces.
if [ $# -lt 2 ]; then
  echo "Usage: $0 <tool-name> <tool-description>"
  exit 1
fi

TOOL_NAME=$1
shift
DESCRIPTION="$*"

# 0) Prerequisites: codex CLI, gh CLI, curl, jq, git, and OPENAI_API_KEY
echo "[0/8] Checking prerequisites..."
for cmd in codex gh curl jq git; do
  if ! command -v "${cmd}" &>/dev/null; then
    echo "Error: '${cmd}' is required but not installed." >&2; exit 1;
  fi
done
if [ -z "${OPENAI_API_KEY-}" ]; then
  echo "Error: OPENAI_API_KEY environment variable not set." >&2; exit 1;
fi
echo "All prerequisites met."

# 1) Generate design spec via ChatGPT API
echo "[1/8] Generating design spec for '${TOOL_NAME}'..."
read -r -d '' DESIGN_PROMPT <<EOF
You are an expert CLI tool designer. Provide a structured design spec for a command-line tool named '${TOOL_NAME}' that ${DESCRIPTION}. Include:
1) Purpose & overview
2) Feature list
3) Example usage
4) Suggested file/directory layout
EOF
curl -s https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer ${OPENAI_API_KEY}" \
  -H "Content-Type: application/json" \
  -d "$(jq -nc --arg m 'gpt-4' --arg msg "${DESIGN_PROMPT}" '{model:$m,messages:[{role:"user",content:$msg}],temperature:0.2}')" \
  | jq -r .choices[0].message.content > design.md
echo "→ design.md created."

# 2) Scaffold project with Codex CLI
echo "[2/8] Scaffolding project '${TOOL_NAME}'..."
mkdir -p "${TOOL_NAME}" && cd "${TOOL_NAME}"
git init
echo "# ${TOOL_NAME}" > README.md
codex --full-auto --project-doc ../design.md -q \
  "Generate a starter CLI project for '${TOOL_NAME}' according to design.md"

# 3) Install dependencies
if [ -f package.json ]; then
  echo "[3/8] Installing npm dependencies..."
  npm install
elif [ -f pyproject.toml ] || [ -f requirements.txt ]; then
  echo "[3/8] Installing Python dependencies..."
  pip install -r requirements.txt
fi

# 4) Create & push GitHub repo
echo "[4/8] Creating GitHub repository..."
gh repo create "${TOOL_NAME}" --public --source=. --remote=origin --push

# 5) Add Codespaces devcontainer
echo "[5/8] Adding .devcontainer config for Codespaces & Copilot..."
mkdir -p .devcontainer
cat > .devcontainer/devcontainer.json <<DEVCON
{
  "name": "${TOOL_NAME} Codespace",
  "image": "mcr.microsoft.com/devcontainers/base:jammy",
  "extensions": ["GitHub.copilot"]
}
DEVCON
git add .devcontainer/devcontainer.json && git commit -m "chore: add Codespace devcontainer"
git push

# 6) Spin up a new GitHub Codespace
echo "[6/8] Spinning up a new GitHub Codespace..."
gh codespace create --repo "$(gh repo view --json nameWithOwner -q .nameWithOwner)"

# 7) Clean up local design file
echo "[7/8] Cleaning up temporary files..."
rm -f design.md

# 8) Integrate ChatGPT Codex web UI
echo "[8/8] Integrating ChatGPT Codex web interface..."
if [[ "${OSTYPE}" == darwin* ]]; then
  BROWSER_CMD="open"
else
  BROWSER_CMD="xdg-open"
fi
ZSHRC="${HOME}/.zshrc"
if ! grep -q "alias codex-web=" "${ZSHRC}" 2>/dev/null; then
  echo "alias codex-web=\"${BROWSER_CMD} https://chatgpt.com/codex\"" >> "${ZSHRC}"
  echo "Alias 'codex-web' added to ${ZSHRC}."
fi
echo "Launching ChatGPT Codex page in browser..."
${BROWSER_CMD} https://chatgpt.com/codex

echo "🎉 Tool '${TOOL_NAME}' scaffolded and environment configured!"