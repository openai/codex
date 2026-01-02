"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: JSONL event writer for runtime observability
Status: Experimental
"""

import json
from pathlib import Path
from typing import Dict, Any, Optional
from datetime import datetime, timezone
import threading


class ReceiptWriter:
    """
    Append-only JSONL writer for runtime events.

    Thread-safe, crash-resistant, never deletes receipts.

    Event types:
    - lesson_start, lesson_complete, lesson_skip
    - op_start, op_done, op_fail
    - attempt_start, attempt_success, attempt_fail
    - cache_hit, cache_miss
    - run_complete
    """

    def __init__(self, receipts_file: str = "celaya/lmu/artifacts/receipts.jsonl"):
        """
        Initialize receipt writer.

        Args:
            receipts_file: Path to JSONL receipts file
        """
        self.receipts_file = Path(receipts_file)
        self.receipts_file.parent.mkdir(parents=True, exist_ok=True)

        # Thread lock for concurrent writes
        self._lock = threading.Lock()

    def emit(self, event: str, **kwargs) -> None:
        """
        Emit a receipt event to JSONL file.

        Args:
            event: Event type (e.g., "lesson_start")
            **kwargs: Event-specific data
        """
        receipt = {
            "event": event,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            **kwargs
        }

        # Write atomically with lock
        with self._lock:
            with open(self.receipts_file, 'a') as f:
                f.write(json.dumps(receipt) + '\n')

    def lesson_start(self, lesson_id: str, **kwargs) -> None:
        """Emit lesson_start event."""
        self.emit("lesson_start", lesson=lesson_id, **kwargs)

    def lesson_complete(self, lesson_id: str, artifacts: list, **kwargs) -> None:
        """Emit lesson_complete event."""
        self.emit("lesson_complete", lesson=lesson_id, artifacts=artifacts, **kwargs)

    def lesson_skip(self, lesson_id: str, reason: str, **kwargs) -> None:
        """Emit lesson_skip event."""
        self.emit("lesson_skip", lesson=lesson_id, reason=reason, **kwargs)

    def op_start(self, op: str, lesson_id: Optional[str] = None, **kwargs) -> None:
        """Emit op_start event."""
        data = {"op": op}
        if lesson_id:
            data["lesson"] = lesson_id
        self.emit("op_start", **data, **kwargs)

    def op_done(
        self,
        op: str,
        duration_ms: int,
        status: str = "success",
        lesson_id: Optional[str] = None,
        **kwargs
    ) -> None:
        """Emit op_done event."""
        data = {"op": op, "duration_ms": duration_ms, "status": status}
        if lesson_id:
            data["lesson"] = lesson_id
        self.emit("op_done", **data, **kwargs)

    def op_fail(
        self,
        op: str,
        reason: str,
        lesson_id: Optional[str] = None,
        **kwargs
    ) -> None:
        """Emit op_fail event."""
        data = {"op": op, "reason": reason}
        if lesson_id:
            data["lesson"] = lesson_id
        self.emit("op_fail", **data, **kwargs)

    def attempt_start(self, op: str, attempt: int, **kwargs) -> None:
        """Emit attempt_start event."""
        self.emit("attempt_start", op=op, attempt=attempt, **kwargs)

    def attempt_success(self, op: str, attempt: int, **kwargs) -> None:
        """Emit attempt_success event."""
        self.emit("attempt_success", op=op, attempt=attempt, **kwargs)

    def attempt_fail(self, op: str, attempt: int, reason: str, **kwargs) -> None:
        """Emit attempt_fail event."""
        self.emit("attempt_fail", op=op, attempt=attempt, reason=reason, **kwargs)

    def cache_hit(self, key: str, **kwargs) -> None:
        """Emit cache_hit event."""
        self.emit("cache_hit", key=key, **kwargs)

    def cache_miss(self, key: str, **kwargs) -> None:
        """Emit cache_miss event."""
        self.emit("cache_miss", key=key, **kwargs)

    def run_complete(
        self,
        lessons_passed: int,
        lessons_failed: int,
        total_duration_ms: int,
        **kwargs
    ) -> None:
        """Emit run_complete event."""
        self.emit(
            "run_complete",
            lessons_passed=lessons_passed,
            lessons_failed=lessons_failed,
            total_duration_ms=total_duration_ms,
            **kwargs
        )


class OperationTimer:
    """
    Context manager for timing operations and emitting receipts.

    Usage:
        with OperationTimer(receipt_writer, "generate_spec", lesson_id="0.1"):
            # do work
            pass
    """

    def __init__(
        self,
        receipt_writer: ReceiptWriter,
        op: str,
        lesson_id: Optional[str] = None,
        **metadata
    ):
        """
        Initialize timer.

        Args:
            receipt_writer: ReceiptWriter instance
            op: Operation name
            lesson_id: Optional lesson identifier
            **metadata: Additional metadata for receipts
        """
        self.receipt_writer = receipt_writer
        self.op = op
        self.lesson_id = lesson_id
        self.metadata = metadata
        self.start_time = None
        self.success = True
        self.error = None

    def __enter__(self):
        """Start timer and emit op_start."""
        self.start_time = datetime.now(timezone.utc)
        self.receipt_writer.op_start(self.op, self.lesson_id, **self.metadata)
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Stop timer and emit op_done or op_fail."""
        duration = datetime.now(timezone.utc) - self.start_time
        duration_ms = int(duration.total_seconds() * 1000)

        if exc_type is not None:
            # Operation failed with exception
            self.receipt_writer.op_fail(
                self.op,
                reason=str(exc_val),
                lesson_id=self.lesson_id,
                **self.metadata
            )
            # Don't suppress exception
            return False
        else:
            # Operation succeeded
            status = "success" if self.success else "failed"
            self.receipt_writer.op_done(
                self.op,
                duration_ms=duration_ms,
                status=status,
                lesson_id=self.lesson_id,
                **self.metadata
            )
            return True

    def mark_failed(self, reason: str):
        """Mark operation as failed without raising exception."""
        self.success = False
        self.error = reason


# Global receipt writer instance
_global_receipt_writer = None


def get_receipt_writer(receipts_file: Optional[str] = None) -> ReceiptWriter:
    """
    Get global receipt writer instance (singleton).

    Args:
        receipts_file: Path to receipts file (only used on first call)

    Returns:
        ReceiptWriter instance
    """
    global _global_receipt_writer

    if _global_receipt_writer is None:
        file = receipts_file or "celaya/lmu/artifacts/receipts.jsonl"
        _global_receipt_writer = ReceiptWriter(file)

    return _global_receipt_writer


# Example usage
if __name__ == "__main__":
    # Initialize writer
    writer = ReceiptWriter("test_receipts.jsonl")

    # Emit events
    writer.lesson_start("0.1", phase="foundations")

    writer.op_start("generate_spec", lesson_id="0.1")
    writer.op_done("generate_spec", duration_ms=1234, lesson_id="0.1")

    writer.attempt_start("validate_json", attempt=1)
    writer.attempt_fail("validate_json", attempt=1, reason="invalid_json")

    writer.attempt_start("validate_json", attempt=2)
    writer.attempt_success("validate_json", attempt=2)

    writer.lesson_complete("0.1", artifacts=["spec.md", "tasks.json"])

    writer.run_complete(lessons_passed=1, lessons_failed=0, total_duration_ms=5000)

    # Use context manager
    with OperationTimer(writer, "complex_operation", lesson_id="0.2"):
        import time
        time.sleep(0.1)  # Simulate work

    print("Receipts written to test_receipts.jsonl")

    # Clean up test file
    Path("test_receipts.jsonl").unlink(missing_ok=True)
