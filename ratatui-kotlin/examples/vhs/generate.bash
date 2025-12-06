#!/bin/bash

# This script is used to generate the images for the examples README
# It requires the following tools:
# - cargo: https://doc.rust-lang.org/cargo/getting-started/installation.html
# - gh: https://github.com/cli/cli
# - git: https://git-scm.com/
# - vhs: https://github.com/charmbracelet/vhs - currently this needs to be installed from the
#   main branch, as the latest release doesn't support the theme we use or the Screenshot
#   command. Install using `go install github.com/charmbracelet/vhs@main`
# - go: https://golang.org/doc/install
# - ttyd: https://github.com/tsl0922/ttyd

# Exit on error. Append "|| true" if you expect an error.
set -o errexit
# Exit on error inside any functions or subshells.
set -o errtrace
# Do not allow use of undefined vars. Use ${VAR:-} to use an undefined VAR
set -o nounset
# Catch the error in case mysqldump fails (but gzip succeeds) in `mysqldump |gzip`
set -o pipefail
# Turn on traces, useful while debugging but commented out by default
# set -o xtrace

# ensure that running each example doesn't have to wait for the build
for app in *; do
    cargo build -p "$app"
done

for tape_path in examples/vhs/*.tape; do
    tape_file=${tape_path/examples\/vhs\//} # strip the examples/vhs/ prefix
    gif_file=${tape_file/.tape/.gif}        # replace the .tape suffix with .gif
    ~/go/bin/vhs "$tape_path" --quiet
    # this can be pasted into the examples README.md
    echo "[${gif_file}]: https://github.com/ratatui/ratatui/blob/images/examples/${gif_file}?raw=true"
done

git switch images
git pull --rebase upstream images
cp target/*.gif examples/
git add examples/*.gif
git commit -m 'docs(examples): update images'
gh pr create
git sw -
