import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import (
    ensure_local_sdk_src,
    runtime_config,
    temporary_sample_image_path,
)

ensure_local_sdk_src()

from openai_codex import Codex, LocalImageInput, TextInput

with temporary_sample_image_path() as image_path:
    with Codex(config=runtime_config()) as codex:
        thread = codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

        result = thread.turn(
            [
                TextInput(
                    "Read this generated local image and summarize the colors/layout in 2 bullets."
                ),
                LocalImageInput(str(image_path.resolve())),
            ]
        ).run()

        print("Status:", result.status)
        print(result.final_response)
