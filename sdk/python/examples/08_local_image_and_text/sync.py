import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

from codex_app_server import Codex, LocalImageInput, TextInput

IMAGE_PATH = Path(__file__).resolve().parents[1] / "assets" / "sample_scene.png"
if not IMAGE_PATH.exists():
    raise FileNotFoundError(f"Missing bundled image: {IMAGE_PATH}")

with Codex(config=runtime_config()) as codex:
    thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

    result = thread.turn(
        [
            TextInput("Read this local image and summarize what you see in 2 bullets."),
            LocalImageInput(str(IMAGE_PATH.resolve())),
        ]
    ).run()

    print("Status:", result.status)
    print(result.text)
