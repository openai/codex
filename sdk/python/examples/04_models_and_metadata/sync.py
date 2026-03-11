import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

from codex_app_server import Codex

with Codex() as codex:
    print("metadata:", codex.metadata)

    models = codex.models()
    print("models.count:", len(models.data))
    if models.data:
        print("first model id:", models.data[0].id)
