#!/bin/bash

set -o errexit -o nounset

rev=$(git rev-parse --short HEAD)

cd target/doc

git init
git config user.email 'FlashCat@users.noreply.github.com'
git config user.name 'FlashCat'
git remote add upstream "https://${GH_TOKEN}@github.com/${TRAVIS_REPO_SLUG}.git"
git fetch upstream gh-pages
git reset upstream/gh-pages

touch .

git add -A .
git commit -m "rebuild pages at ${rev}"
git push -q upstream HEAD:gh-pages
