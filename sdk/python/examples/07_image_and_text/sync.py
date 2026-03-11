import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

from codex_app_server import Codex, ImageInput, TextInput

REMOTE_IMAGE_URL = "https://raw.githubusercontent.com/github/explore/main/topics/python/python.png"

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    result = thread.turn(
        [
            TextInput("What is in this image? Give 3 bullets."),
            ImageInput(REMOTE_IMAGE_URL),
        ]
    ).run()

    print("Status:", result.status)
    print(result.text)
