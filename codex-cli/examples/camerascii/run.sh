#!/bin/bash

# run.sh — Create a new run_N directory for a Codex task, optionally bootstrapped from a template,
# then launch Codex with the task description from task.yaml.
#
# Usage:
#   ./run.sh                  # Prompts to confirm new run
#   ./run.sh --auto-confirm   # Skips confirmation
#
# Assumes:
#   - yq and jq are installed
#   - ../task.yaml exists (with .name and .description fields)
#   - ../template/ exists (optional, for bootstrapping new runs)

# Enable auto-confirm mode if flag is passed
auto_mode=false
[[ "$1" == "--auto-confirm" ]] && auto_mode=true

# Create the runs directory if it doesn't exist
mkdir -p runs

# Move into the working directory
cd runs || exit 1

# Grab task name for logging
task_name=$(yq -o=json '.' ../task.yaml | jq -r '.name')
echo "Checking for runs for task: $task_name"

# Find existing run_N directories
shopt -s nullglob
run_dirs=(run_[0-9]*)
shopt -u nullglob

if [ ${#run_dirs[@]} -eq 0 ]; then
  echo "There are 0 runs."
  new_run_number=1
else
  max_run_number=0
  for d in "${run_dirs[@]}"; do
    [[ "$d" =~ ^run_([0-9]+)$ ]] && (( ${BASH_REMATCH[1]} > max_run_number )) && max_run_number=${BASH_REMATCH[1]}
  done
  new_run_number=$((max_run_number + 1))
  echo "There are $max_run_number runs."
fi

# Confirm creation unless in auto mode
if [ "$auto_mode" = false ]; then
  read -p "Create run_$new_run_number? (Y/N): " choice
  [[ "$choice" != [Yy] ]] && echo "Exiting." && exit 1
fi

# Create the run directory
mkdir "run_$new_run_number"

# Check if the template directory exists and copy its contents
if [ -d "../template" ]; then
  cp -r ../template/* "run_$new_run_number"
  echo "Initialized run_$new_run_number from template/"
else
  echo "Template directory does not exist. Skipping initialization from template."
fi

cd "run_$new_run_number"

# Launch Codex
echo "Launching..."
description=$(yq -o=json '.' ../../task.yaml | jq -r '.description')
codex "$description"
