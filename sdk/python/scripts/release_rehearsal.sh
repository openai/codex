#!/usr/bin/env bash
set -euo pipefail

# TestPyPI rehearsal script for codex-app-server-sdk.
# Usage:
#   scripts/release_rehearsal.sh
# Optional env for upload:
#   TWINE_USERNAME=__token__
#   TWINE_PASSWORD=pypi-***

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f ".venv2/bin/activate" ]]; then
  # shellcheck disable=SC1091
  source .venv2/bin/activate
fi

python -m build
python -m twine check dist/*

if [[ -n "${TWINE_USERNAME:-}" && -n "${TWINE_PASSWORD:-}" ]]; then
  echo "Uploading to TestPyPI..."
  python -m twine upload \
    --repository-url https://test.pypi.org/legacy/ \
    dist/*

  TMP_VENV="/tmp/codex_sdk_testpypi_venv"
  rm -rf "$TMP_VENV"
  python -m venv "$TMP_VENV"
  # shellcheck disable=SC1091
  source "$TMP_VENV/bin/activate"
  python -m pip install -U pip >/dev/null
  python -m pip install -i https://test.pypi.org/simple/ --extra-index-url https://pypi.org/simple codex-app-server-sdk
  python - <<'PY'
import codex_app_server as m
print('installed_from_testpypi', m.__version__)
PY
else
  echo "TWINE credentials not set; skipping upload step."
  echo "Set TWINE_USERNAME and TWINE_PASSWORD to run full TestPyPI rehearsal."
fi
