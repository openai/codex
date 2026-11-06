#!/usr/bin/env bash
set -euo pipefail

repo_api="repos/$GITHUB_REPOSITORY"
# Look up this tag, including drafts. A lookup error must abort publication.
# shellcheck disable=SC2016
exists="$(gh api graphql \
  -f query='query($owner:String!,$repo:String!,$tag:String!){repository(owner:$owner,name:$repo){release(tagName:$tag){databaseId}}}' \
  -f owner="$GITHUB_REPOSITORY_OWNER" \
  -f repo="${GITHUB_REPOSITORY#*/}" \
  -f tag="$GITHUB_REF_NAME" \
  --jq '.data.repository.release != null')"

if [[ "$exists" == false ]]; then
  # The create API would create a missing tag from the default branch.
  gh api --silent "$repo_api/git/ref/tags/$GITHUB_REF_NAME"
  draft=true
  if [[ "$PRERELEASE" == true ]]; then
    # Create prereleases directly to preserve the prereleased event.
    draft=false
  fi
  release_id="$(gh api --method POST "$repo_api/releases" \
    -f tag_name="$GITHUB_REF_NAME" -f name="$RELEASE_NAME" -F "body=@$RELEASE_NOTES" \
    -F draft="$draft" -F prerelease="$PRERELEASE" -f make_latest=false --jq '.id')"
  if [[ "$PRERELEASE" == true ]]; then
    # A maintainer can replace a release under the same tag during upload.
    trap '
      echo "Removing incomplete prerelease $release_id" >&2
      gh api --method DELETE "$repo_api/releases/$release_id" || true
    ' EXIT
  fi
fi

mapfile -d '' assets < <(find dist -type f -print0 | sort -z)
gh release upload "$GITHUB_REF_NAME" "${assets[@]}" --repo "$GITHUB_REPOSITORY" --clobber
trap - EXIT

gh release edit "$GITHUB_REF_NAME" --repo "$GITHUB_REPOSITORY" \
  --title "$RELEASE_NAME" --notes-file "$RELEASE_NOTES" --prerelease="$PRERELEASE" \
  --draft=false --latest="$MAKE_LATEST"
