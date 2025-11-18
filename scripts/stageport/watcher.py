"""Watch a curriculum file and stream validation reports."""
from __future__ import annotations

import os
import time
from pathlib import Path

from watchdog.events import FileSystemEventHandler
from watchdog.observers import Observer

from stageport_evolution import validate

TARGET_FILE = os.environ.get("STAGEPORT_TARGET", "your_curriculum.txt")
LEDGER_PATH = os.environ.get("STAGEPORT_LEDGER")
LOG_MASTER = os.environ.get("STAGEPORT_LOG_MASTER") == "1"
FIREBASE_CREDENTIALS = os.environ.get("STAGEPORT_FIREBASE_CREDENTIALS")
USER_ID = os.environ.get("STAGEPORT_USER_ID", "dev-user")


class DanceNoteHandler(FileSystemEventHandler):
    def on_modified(self, event):
        if event.is_directory:
            return
        if Path(event.src_path).name != TARGET_FILE:
            return

        curriculum = Path(TARGET_FILE).read_text(encoding="utf-8")
        report = validate(
            curriculum,
            ledger_path=LEDGER_PATH,
            log_master=LOG_MASTER,
            user_id=USER_ID,
            firebase_credentials=FIREBASE_CREDENTIALS,
        )

        print("\n--- New Evaluation ---")
        print(report)


if __name__ == "__main__":
    target_path = Path(TARGET_FILE)
    print(f"Watching {target_path} for live evolution...")
    event_handler = DanceNoteHandler()
    observer = Observer()
    observer.schedule(event_handler, target_path.parent, recursive=False)
    observer.start()
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        observer.stop()
    observer.join()
