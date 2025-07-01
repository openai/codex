#!/usr/bin/env bash
# Exit immediately on error, treat unset variables as errors, and catch failures
set -euo pipefail

# ---------------------------------------------------------------
# Codex CLI Initialization Helper
# ---------------------------------------------------------------
# This script inspects your project and then asks OpenAI to write a
# starter `AGENT.md` for using Codex CLI. It can optionally query
# Perplexity for extra tips.

# jq is used for parsing JSON responses from the APIs. Abort if missing.
if ! command -v jq >/dev/null; then
  echo "Error: jq is required" >&2
  exit 1
fi

# Where are we running? We'll capture the current directory so the
# instructions reference the right path. Temporary files hold our
# analysis and questionnaire answers.
PROJECT_DIR=$(pwd)
ANALYSIS_FILE=$(mktemp)

# ---------------------------------------------------------------
# Gather a quick summary of the project so the model knows what it
# is looking at. We collect the directory path, the current date, and
# a short file listing.
# ---------------------------------------------------------------
{
  echo "# Project Analysis"
  echo "Path: $PROJECT_DIR"
  echo "Date: $(date)"
  echo
  echo "## File listing"
  find . -maxdepth 2 -type f | head -n 100
} > "$ANALYSIS_FILE"

# Append dependency information if we find Node or Python manifests
if [ -f package.json ]; then
  echo >> "$ANALYSIS_FILE"
  echo '## package.json' >> "$ANALYSIS_FILE"
  cat package.json >> "$ANALYSIS_FILE"
fi
if [ -f requirements.txt ]; then
  echo >> "$ANALYSIS_FILE"
  echo '## requirements.txt' >> "$ANALYSIS_FILE"
  cat requirements.txt >> "$ANALYSIS_FILE"
fi

# ---------------------------------------------------------------
# Simple yes/no questionnaire so the model can customize the output
# based on your preferences.
# ---------------------------------------------------------------
read -p "Enable Docker setup? (y/n) " DOCKER_Q
read -p "Create .env.example? (y/n) " ENV_Q
read -p "Include sandbox instructions? (y/n) " SANDBOX_Q

# Store the answers in a temporary file for easy inclusion in the prompt
QUESTIONNAIRE_FILE=$(mktemp)
{
  echo "docker: $DOCKER_Q"
  echo "env_example: $ENV_Q"
  echo "sandbox: $SANDBOX_Q"
} > "$QUESTIONNAIRE_FILE"

# The system prompt establishes the role for the LLM
SYSTEM_PROMPT="You generate helpful AGENT.md files for projects using Codex CLI."

# ---------------------------------------------------------------
# Optionally fetch Perplexity research to enrich instructions.
# This step uses the Perplexity API if PPLX_API_KEY is set.
# ---------------------------------------------------------------
PPLX_INFO=""
if [ -n "${PPLX_API_KEY:-}" ]; then
  PROJECT_SUMMARY=$(tr '\n' ' ' < "$ANALYSIS_FILE")
  PPLX_RESPONSE=$(curl -s https://api.perplexity.ai/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $PPLX_API_KEY" \
    -d "{\n      \"model\": \"pplx-70b-online\",\n      \"messages\": [\n        {\"role\": \"user\", \"content\": \"Summarize best practices for setting up Codex CLI agents based on the following project analysis: $PROJECT_SUMMARY\"}\n      ],\n      \"max_tokens\": 200\n    }")
  # Extract the plain text response from Perplexity
  PPLX_INFO=$(echo "$PPLX_RESPONSE" | jq -r '.choices[0].message.content // empty')
  if [ -n "$PPLX_INFO" ]; then
    echo "Perplexity research added" >&2
  fi
else
  echo "PPLX_API_KEY not set; skipping Perplexity research" >&2
fi

# Build the user prompt combining the analysis, questionnaire and any research
USER_PROMPT="Project details:\n$(cat "$ANALYSIS_FILE")\n\nQuestionnaire:\n$(cat "$QUESTIONNAIRE_FILE")"
if [ -n "$PPLX_INFO" ]; then
  USER_PROMPT+="\n\nPerplexity insights:\n$PPLX_INFO"
fi
USER_PROMPT+="\n\nWrite AGENT.md instructions."

if [ -z "${OPENAI_API_KEY:-}" ]; then
  echo "OPENAI_API_KEY not set; skipping API call" >&2
else
  # Ask OpenAI to draft the AGENT.md based on all gathered context
  RESPONSE=$(curl -s https://api.openai.com/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $OPENAI_API_KEY" \
    -d "{\n      \"model\": \"gpt-4o\",\n      \"messages\": [\n        {\"role\": \"system\", \"content\": $(jq -Rs . <<<\"$SYSTEM_PROMPT\")},\n        {\"role\": \"user\", \"content\": $(jq -Rs . <<<\"$USER_PROMPT\")}\n      ],\n      \"max_tokens\": 1200,\n      \"temperature\": 0.2\n    }")
  AGENT_CONTENT=$(echo "$RESPONSE" | jq -r '.choices[0].message.content // empty')
  if [ -n "$AGENT_CONTENT" ]; then
    # Save the model's output directly to AGENT.md in the project root
    echo "$AGENT_CONTENT" > AGENT.md
    echo "Generated AGENT.md" >&2
  else
    echo "Failed to generate AGENT.md" >&2
  fi
fi

# Optionally create helper files based on your answers
if [[ "$ENV_Q" =~ ^[Yy]$ ]]; then
  cat > .env.example <<'EOT'
# Example environment variables
OPENAI_API_KEY=your-key-here
EOT
  echo "Created .env.example" >&2
fi

if [[ "$DOCKER_Q" =~ ^[Yy]$ ]]; then
  # Create a very small setup script so users can install dependencies later
  cat > setup.sh <<'EOT'
#!/usr/bin/env bash
# Basic setup script
if [ -f package.json ]; then
  pnpm install
fi
EOT
  chmod +x setup.sh
  echo "Created setup.sh" >&2
fi

# Remove temporary files before exiting
rm -f "$ANALYSIS_FILE" "$QUESTIONNAIRE_FILE"
