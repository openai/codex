#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/notarize.sh artifact1 artifact2 ... -o out_bundle.zip
# Example:
#   ./scripts/notarize.sh PyRouette_report.tex selfimproving_guidance.py cunning_mercy.html -o batch1_bundle.zip

OUT_ZIP="bundle_$(date +%Y%m%d_%H%M%S).zip"
FILES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -o|--out)
      OUT_ZIP="$2"
      shift 2
      ;;
    *)
      FILES+=("$1")
      shift
      ;;
  esac
done

if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "No files provided. Usage: ./scripts/notarize.sh file1 file2 ... -o out.zip"
  exit 1
fi

for f in "${FILES[@]}"; do
  if [[ ! -f "$f" ]]; then
    echo "Missing file: $f" >&2
    exit 1
  fi
done

zip -r "$OUT_ZIP" "${FILES[@]}"

declare -a HASH_CMD
if command -v sha256sum >/dev/null 2>&1; then
  HASH_CMD=(sha256sum "$OUT_ZIP")
elif command -v shasum >/dev/null 2>&1; then
  HASH_CMD=(shasum -a 256 "$OUT_ZIP")
else
  echo "Neither sha256sum nor shasum found. Install one to continue." >&2
  exit 1
fi

SHA256=$("${HASH_CMD[@]}" | awk '{print $1}')
{
  echo "Bundle: $OUT_ZIP"
  echo "SHA256: $SHA256"
} > "${OUT_ZIP}.sha256"

echo "Bundle: $OUT_ZIP"
echo "SHA256: $SHA256 (saved to ${OUT_ZIP}.sha256)"

if command -v gpg >/dev/null 2>&1; then
  gpg --armor --detach-sign "${OUT_ZIP}.sha256" || true
  echo "GPG signature written: ${OUT_ZIP}.sha256.asc"
fi

CID=""
if command -v ipfs >/dev/null 2>&1; then
  CID=$(ipfs add -Q "$OUT_ZIP")
  echo "Pinned to IPFS. CID: $CID"
  echo "$CID" > "${OUT_ZIP}.cid"
else
  echo "ipfs CLI not found; skipping IPFS pin. Install go-ipfs or use a pinning service to capture a CID."
fi

echo "To notarize on-chain, run: node scripts/notarize.cjs --hash 0x${SHA256} ${CID:+--cid ${CID}}"
