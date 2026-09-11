"""Capture one bounded diagnostic when the ARM64 voice build goes silent."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time


def diagnose():
    root = Path(os.environ.get("BAZEL_OUTPUT_BASE", ""))
    if not root.is_absolute():
        print("Bazel output base unavailable; skipping diagnostics", flush=True)
        return
    for relative in ("command.log", "server/jvm.out"):
        try:
            with (root / relative).open("rb") as source:
                source.seek(max(0, source.seek(0, 2) - 16384))
                print(
                    f"Bazel {relative} tail:\n{source.read(16384).decode(errors='replace')}",
                    flush=True,
                )
        except OSError as error:
            print(f"Cannot read {relative}: {error}", flush=True)
    try:
        pid = str(int((root / "server/server.pid.txt").read_text().strip()))
        jstack = shutil.which("jstack")
        if not jstack:
            print("jstack unavailable; log tails retained", flush=True)
            return
        result = subprocess.run([jstack, pid], capture_output=True, timeout=20)
        print(
            f"jstack exit {result.returncode}:\n{(result.stdout + result.stderr)[-65536:].decode(errors='replace')}",
            flush=True,
        )
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"Cannot capture JVM stacks: {error}", flush=True)


def main():
    with subprocess.Popen(
        sys.argv[1:], stdout=subprocess.PIPE, stderr=subprocess.STDOUT
    ) as process:
        last_output = time.monotonic()

        def forward():
            nonlocal last_output
            for line in process.stdout:
                last_output = time.monotonic()
                sys.stdout.buffer.write(line)
                sys.stdout.buffer.flush()

        reader = threading.Thread(target=forward)
        reader.start()
        captured = False
        while process.poll() is None:
            if (
                not captured
                and os.environ.get("VOICE_ARCH") == "aarch64"
                and time.monotonic() - last_output >= 600
            ):
                captured = True
                print(
                    "ARM64 Bazel silent for ten minutes; capturing diagnostics",
                    flush=True,
                )
                diagnose()
            time.sleep(1)
        reader.join()
        if process.returncode and not captured:
            print("Bazel command failed; capturing diagnostics", flush=True)
            diagnose()
        return process.returncode


if __name__ == "__main__":
    sys.exit(main())
