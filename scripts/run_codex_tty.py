#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pyte"]
# ///
import argparse
import os
import queue
import select
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

try:
    import pyte
except ImportError as exc:
    raise SystemExit(
        "pyte is required. Install with: pip install pyte"
    ) from exc


ENTER = b"\r"
STEER_OVERRIDES = [
    "--config",
    "features.collaboration_modes=true",
    "--config",
    "tui.experimental_mode=plan",
]
TYPE_DELAY_SEC = 0.02


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run N Codex TTY instances, send a prompt, and capture ASCII screenshots."
    )
    parser.add_argument("prompt", help="Prompt to send to each Codex instance.")
    parser.add_argument(
        "-n",
        "--instances",
        type=int,
        default=1,
        help="Number of Codex instances to launch.",
    )
    parser.add_argument(
        "--out-dir",
        default="screens",
        help="Directory to write ASCII screenshots.",
    )
    parser.add_argument(
        "--cols",
        type=int,
        default=120,
        help="TTY columns for each instance.",
    )
    parser.add_argument(
        "--rows",
        type=int,
        default=1200,
        help="TTY rows for each instance.",
    )
    return parser.parse_args()


def build_codex(repo_root: Path) -> Path:
    codex_rs = repo_root / "codex-rs"
    result = subprocess.run(
        ["cargo", "build", "-p", "codex-cli"],
        cwd=codex_rs,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit("cargo build failed")
    codex_bin = codex_rs / "target" / "debug" / "codex"
    if not codex_bin.exists():
        raise SystemExit(f"codex binary not found at {codex_bin}")
    return codex_bin


def set_winsize(fd: int, rows: int, cols: int) -> None:
    import fcntl
    import struct
    import termios

    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


def run_instance(
    index: int,
    codex_bin: Path,
    repo_root: Path,
    prompt: str,
    out_dir: Path,
    rows: int,
    cols: int,
    result_queue: "queue.Queue[tuple[int, Path]]",
) -> None:
    master_fd, slave_fd = os.openpty()
    set_winsize(slave_fd, rows, cols)

    env = os.environ.copy()
    env["COLUMNS"] = str(cols)
    env["LINES"] = str(rows)

    proc = subprocess.Popen(
        [str(codex_bin), *STEER_OVERRIDES],
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=slave_fd,
        env=env,
        cwd=repo_root,
        preexec_fn=os.setsid,
        close_fds=True,
    )
    os.close(slave_fd)

    screen = pyte.Screen(cols, rows)
    stream = pyte.Stream(screen)

    last_output = time.monotonic()
    sent_prompt = False
    start_time = time.monotonic()
    prompt_attempt_deadline = start_time + 5.0
    prompt_sent_at: float | None = None
    response_started = False
    last_enter_retry = 0.0
    buffer = b""
    cpr_probe = b""

    try:
        while True:
            if not sent_prompt and time.monotonic() - start_time >= 1.0:
                prompt_line = next((line for line in screen.display if "›" in line), "")
                has_prompt = bool(prompt_line)
                if has_prompt or time.monotonic() >= prompt_attempt_deadline:
                    write_with_delay(master_fd, prompt.encode(), TYPE_DELAY_SEC)
                    time.sleep(0.2)
                    os.write(master_fd, ENTER)
                    sent_prompt = True
                    prompt_sent_at = time.monotonic()

            timeout = 0.1
            rlist, _, _ = select.select([master_fd], [], [], timeout)
            if rlist:
                data = os.read(master_fd, 4096)
                if not data:
                    break
                last_output = time.monotonic()
                cpr_probe = (cpr_probe + data)[-8:]
                if b"\x1b[6n" in cpr_probe:
                    os.write(master_fd, b"\x1b[1;1R")
                buffer += data
                while True:
                    try:
                        chunk = buffer.decode(errors="ignore")
                    except UnicodeDecodeError:
                        break
                    stream.feed(chunk)
                    if not response_started and any("Working" in line for line in screen.display):
                        response_started = True
                    buffer = b""
                    break

            if sent_prompt and not response_started:
                now = time.monotonic()
                if now - prompt_sent_at >= 2.0 and now - last_enter_retry >= 2.0:
                    os.write(master_fd, ENTER)
                    last_enter_retry = now

            if response_started and time.monotonic() - last_output >= 10.0:
                break

            if proc.poll() is not None:
                break
    finally:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except:
            pass
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except:
                pass

        out_dir.mkdir(parents=True, exist_ok=True)
        out_path = out_dir / f"codex_{index}.txt"
        with out_path.open("w", encoding="utf-8") as f:
            text = ""
            for line in screen.display:
                text += line.rstrip() + "\n"

            text = text.rstrip("\n ")
            f.write(text)


        print(f"done {index}: {out_path}")
        result_queue.put((index, out_path))

        os.close(master_fd)


def main() -> int:
    args = parse_args()
    if args.instances < 1:
        raise SystemExit("--instances must be >= 1")

    repo_root = Path(__file__).resolve().parents[1]
    codex_bin = build_codex(repo_root)

    out_dir = Path(args.out_dir)
    result_queue: "queue.Queue[tuple[int, Path]]" = queue.Queue()
    threads = []
    for i in range(1, args.instances + 1):
        thread = threading.Thread(
            target=run_instance,
            args=(
                i,
                codex_bin,
                repo_root,
                args.prompt,
                out_dir,
                args.rows,
                args.cols,
                result_queue,
            ),
            daemon=True,
        )
        thread.start()
        threads.append(thread)

    for thread in threads:
        thread.join()

    sorted(result_queue.get() for _ in range(args.instances))
    return 0


def write_with_delay(fd: int, data: bytes, delay: float = 0.05) -> None:
    for ch in data:
        os.write(fd, bytes([ch]))
        time.sleep(delay)


if __name__ == "__main__":
    sys.exit(main())
