import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config, server_label

ensure_local_sdk_src()

from codex_app_server import Codex

with Codex(config=runtime_config()) as codex:
    print("server:", server_label(codex.metadata))
    models = codex.models()
    print("models.count:", len(models.data))
    print("models:", ", ".join(model.id for model in models.data[:5]) or "[none]")
