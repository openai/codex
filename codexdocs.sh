#!/bin/bash
###############################################################################
# File:        codexdocs.sh
# Purpose:     Launch the Codex CLI on a user-selected documentation folder.
# Author:      d33disc (generated with GitHub Copilot)
# Date:        2025-06-27
#
# Description:
#   A beginner-friendly Bash script to help you run the Codex CLI on any
#   documentation folder, with a graphical folder picker (for macOS) and
#   prompts for your main documentation file. The script checks if required
#   files exist and provides clear error messages. Designed for easy use
#   and clarity for beginner programmers.
#
# Usage:
#   1. Save this script in the root of your Codex repository as codexdocs.sh.
#   2. Make it executable: chmod +x codexdocs.sh
#   3. Run: ./codexdocs.sh
#
# Requirements:
#   - macOS (for AppleScript folder picker)
#   - Codex CLI installed and accessible via `codex` in your PATH
#
# Notes:
#   - This script is safe to run multiple times.
#   - If you are on Linux or Windows, replace the folder picker section
#     with an appropriate method for your OS.
###############################################################################

# Exit on errors
set -e

# Function to print a separator line
print_separator() {
  echo
  echo "-----------------------------------------------------------"
  echo
}

# 1. Use AppleScript to prompt the user to select their documentation folder.
print_separator
echo "Welcome to CodexDocs!"
echo "Please select your documentation folder (where your docs live)."
print_separator

DOCS_FOLDER=$(osascript <<EOT
  set folderPath to POSIX path of (choose folder with prompt "Select your documentation folder for Codex:")
  return folderPath
EOT
)

# Remove trailing slash if present
DOCS_FOLDER="${DOCS_FOLDER%/}"

# Check if folder exists
if [ ! -d "$DOCS_FOLDER" ]; then
  echo "Error: The selected docs folder does not exist: $DOCS_FOLDER"
  exit 1
fi

echo "You selected: $DOCS_FOLDER"

# 2. Ask for the main documentation file name (default: index.md)
read -p "Enter your main documentation file name (default: index.md): " MAIN_DOC_FILE
MAIN_DOC_FILE=${MAIN_DOC_FILE:-index.md}

# Check if the main doc file exists in the selected folder
if [ ! -f "$DOCS_FOLDER/$MAIN_DOC_FILE" ]; then
  echo "Error: File '$MAIN_DOC_FILE' not found in $DOCS_FOLDER."
  echo "Please make sure the file exists, then re-run this script."
  exit 1
fi

echo "Main documentation file: $MAIN_DOC_FILE"

# 3. Run the Codex CLI with the chosen folder and file
print_separator
echo "Launching Codex CLI with:"
echo "  - Docs folder: $DOCS_FOLDER"
echo "  - Main file:   $MAIN_DOC_FILE"
print_separator

# Construct the Codex CLI command
CODEX_CMD="codex --docs-folder \"$DOCS_FOLDER\" --main-doc \"$MAIN_DOC_FILE\""

# Show the command for transparency
echo "Running command:"
echo "$CODEX_CMD"

# Actually run the command
eval $CODEX_CMD

print_separator
echo "Codex CLI has finished running."
echo "If you see errors above, please review them and try again."
echo "Thank you for using CodexDocs!"
print_separator

exit 0