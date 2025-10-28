#!/usr/bin/env bash
set -euo pipefail

# Generate a minimal Homebrew formula from the latest GitHub release.
# Writes Formula/Code.rb into the repo root (not a tap); you can copy it
# into a tap repo to publish.

owner_repo="just-every/code"
version="${1:-}"

# Helper to treat placeholder versions as unset
sanitize_version() {
  case "$1" in
    ""|"0.0.0"|"0.0.0-dev") echo "" ;;
    *) echo "$1" ;;
  esac
}

version="$(sanitize_version "$version")"

if [ -z "$version" ] && [ -f "code-rs/Cargo.toml" ]; then
  raw="$(awk -F '"' '/^\[workspace.package\]/{f=1; next} f && $1 ~ /version/ {print $2; exit}' code-rs/Cargo.toml)"
  version="$(sanitize_version "$raw")"
fi
if [ -z "$version" ] && [ -f "codex-cli/package.json" ]; then
  raw="$(jq -r .version codex-cli/package.json)"
  version="$(sanitize_version "$raw")"
fi
if [ -z "$version" ] && git rev-parse --git-dir >/dev/null 2>&1; then
  tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"
  if [ -n "$tag" ]; then
    # Strip optional leading 'v'
    tag="${tag#v}"
    version="$(sanitize_version "$tag")"
  fi
fi
if [ -z "$version" ]; then
  cat <<'ERR' >&2
Unable to infer release version.
Provide it as the first argument or ensure a tagged release exists.
The script no longer defaults to 0.0.0 to avoid publishing broken formulas.
ERR
  exit 1
fi

# Optional directory where CI placed artifacts (step: Prepare release assets)
RELEASE_ASSETS_DIR=${RELEASE_ASSETS_DIR:-"release-assets"}

assets=(
  "code-aarch64-apple-darwin.tar.gz"
  "code-x86_64-apple-darwin.tar.gz"
)

sha256_file() {
  local f="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  else
    echo ""; return 1
  fi
}

# Retry a command with backoff: retry <max_tries> <sleep_seconds> <cmd...>
retry() {
  local max="$1"; shift
  local sleep_s="$1"; shift
  local n=1
  while :; do
    if "$@"; then return 0; fi
    if [ "$n" -ge "$max" ]; then return 1; fi
    n=$((n+1))
    sleep "$sleep_s"
  done
}

mkdir -p Formula
cat > Formula/Code.rb <<'RUBY'
class Code < Formula
  desc "Terminal coding agent"
  homepage "https://github.com/just-every/code"
RUBY

echo "  version \"v${version}\"" >> Formula/Code.rb

cat >> Formula/Code.rb <<'RUBY'
  on_macos do
    if Hardware::CPU.arm?
      url "__URL_ARM64__"
      sha256 "__SHA_ARM64__"
    else
      url "__URL_X64__"
      sha256 "__SHA_X64__"
    end
  end

  def install
    bin.install Dir["code-*"].first => "code"
    # Provide a compatibility shim
    (bin/"coder").write <<~EOS
      #!/bin/bash
      exec "#{bin}/code" "$@"
    EOS
  end

  test do
    system "#{bin}/code", "--help"
  end
end
RUBY

for a in "${assets[@]}"; do
  url="https://github.com/${owner_repo}/releases/download/v${version}/${a}"
  tmp="/tmp/${a}"
  sha=""

  # Prefer local artifact if available to avoid CDN propagation races
  local_path="${RELEASE_ASSETS_DIR}/${a}"
  if [ -f "$local_path" ]; then
    echo "Using local asset for sha256: ${local_path}" >&2
    sha=$(sha256_file "$local_path") || sha=""
  fi

  # Fallback to remote download (with retries) if local missing or sha empty
  if [ -z "$sha" ]; then
    echo "Downloading ${url} (fallback for sha256)..." >&2
    if ! retry 12 5 curl -fsSL "${url}" -o "${tmp}"; then
      echo "WARN: Could not download ${url} to compute sha256 (possible CDN delay)." >&2
      echo "      Proceeding without sha; Homebrew step will still push formula referencing the URL." >&2
    else
      sha=$(sha256_file "$tmp" || true)
    fi
  fi

  # Apply URL (always), and sha when available
  if [[ "${a}" == *"aarch64-apple-darwin"* ]]; then
    sed -i.bak "s#__URL_ARM64__#${url}#" Formula/Code.rb
    if [ -n "$sha" ]; then sed -i.bak "s#__SHA_ARM64__#${sha}#" Formula/Code.rb; fi
  else
    sed -i.bak "s#__URL_X64__#${url}#" Formula/Code.rb
    if [ -n "$sha" ]; then sed -i.bak "s#__SHA_X64__#${sha}#" Formula/Code.rb; fi
  fi
done

rm -f Formula/Code.rb.bak
echo "Wrote Formula/Code.rb for v${version}" >&2

# Optional: best-effort HEAD check to surface propagation status without failing CI
for a in "${assets[@]}"; do
  url="https://github.com/${owner_repo}/releases/download/v${version}/${a}"
  if ! retry 6 5 bash -c "curl -fsI \"$url\" >/dev/null"; then
    echo "WARN: ${a} not yet available at ${url} (HEAD 404). Likely CDN propagation; continuing." >&2
  fi
done
