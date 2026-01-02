"""
Organization: Celaya Solutions
Project: LMU Curriculum Runtime
Version: 0.1.0
Generated: 2026-01-02T18:54:00Z
Purpose: Named operations, timing, and isolation for LMU execution
Status: Experimental
"""

import subprocess
import json
from pathlib import Path
from typing import Dict, Any, Optional, Callable, List
from datetime import datetime, timezone
import sys

from .receipts import ReceiptWriter, OperationTimer, get_receipt_writer


class LMURunner:
    """
    Execute LMU operations with isolation, retries, and receipts.

    Operations are named, timed, and logged.
    Failures are recorded but don't crash the runner.
    """

    def __init__(
        self,
        receipts_file: str = "celaya/lmu/artifacts/receipts.jsonl",
        max_retries: int = 3,
        timeout_ms: int = 30000
    ):
        """
        Initialize LMU runner.

        Args:
            receipts_file: Path to receipts JSONL file
            max_retries: Maximum retry attempts per operation
            timeout_ms: Operation timeout in milliseconds
        """
        self.receipt_writer = get_receipt_writer(receipts_file)
        self.max_retries = max_retries
        self.timeout_ms = timeout_ms

    def run_operation(
        self,
        op_name: str,
        op_func: Callable,
        lesson_id: Optional[str] = None,
        retry_on_failure: bool = True,
        **kwargs
    ) -> tuple[bool, Any, Optional[str]]:
        """
        Run a named operation with retries and receipts.

        Args:
            op_name: Operation name (e.g., "generate_spec")
            op_func: Function to execute
            lesson_id: Optional lesson identifier
            retry_on_failure: Whether to retry on failure
            **kwargs: Arguments to pass to op_func

        Returns:
            Tuple of (success, result, error_message)
        """
        attempt = 1
        max_attempts = self.max_retries if retry_on_failure else 1

        while attempt <= max_attempts:
            try:
                # Emit attempt_start
                self.receipt_writer.attempt_start(op_name, attempt, lesson=lesson_id)

                # Execute operation with timer
                with OperationTimer(self.receipt_writer, op_name, lesson_id) as timer:
                    result = op_func(**kwargs)

                # Success
                self.receipt_writer.attempt_success(op_name, attempt, lesson=lesson_id)
                return True, result, None

            except Exception as e:
                # Failure
                error_msg = str(e)
                self.receipt_writer.attempt_fail(
                    op_name,
                    attempt,
                    reason=error_msg,
                    lesson=lesson_id
                )

                if attempt >= max_attempts:
                    # No more retries
                    return False, None, f"Failed after {attempt} attempts: {error_msg}"

                # Retry with tightened constraints
                # (Constraint tightening logic would go in op_func)
                attempt += 1

        return False, None, "Max retries exceeded"

    def run_command(
        self,
        command: List[str],
        cwd: Optional[str] = None,
        env: Optional[Dict[str, str]] = None,
        op_name: str = "run_command",
        lesson_id: Optional[str] = None
    ) -> tuple[bool, str, str]:
        """
        Run a shell command with isolation and receipts.

        Args:
            command: Command and arguments as list
            cwd: Working directory
            env: Environment variables
            op_name: Operation name for receipts
            lesson_id: Optional lesson identifier

        Returns:
            Tuple of (success, stdout, stderr)
        """
        with OperationTimer(self.receipt_writer, op_name, lesson_id) as timer:
            try:
                result = subprocess.run(
                    command,
                    cwd=cwd,
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=self.timeout_ms / 1000
                )

                if result.returncode != 0:
                    timer.mark_failed(f"Exit code {result.returncode}")
                    return False, result.stdout, result.stderr

                return True, result.stdout, result.stderr

            except subprocess.TimeoutExpired:
                timer.mark_failed("Timeout")
                return False, "", "Command timed out"

            except Exception as e:
                timer.mark_failed(str(e))
                return False, "", str(e)

    def run_python_script(
        self,
        script_path: str,
        args: Optional[List[str]] = None,
        lesson_id: Optional[str] = None
    ) -> tuple[bool, str, str]:
        """
        Run a Python script with isolation.

        Args:
            script_path: Path to Python script
            args: Command-line arguments
            lesson_id: Optional lesson identifier

        Returns:
            Tuple of (success, stdout, stderr)
        """
        command = [sys.executable, script_path]
        if args:
            command.extend(args)

        return self.run_command(
            command,
            op_name=f"run_script:{Path(script_path).name}",
            lesson_id=lesson_id
        )

    def run_lesson_script(
        self,
        lesson_dir: str,
        lesson_id: str
    ) -> tuple[bool, Dict[str, Any]]:
        """
        Run a lesson's run.sh or run.py script.

        Args:
            lesson_dir: Directory containing lesson artifacts
            lesson_id: Lesson identifier

        Returns:
            Tuple of (success, summary_dict)
        """
        lesson_path = Path(lesson_dir)

        # Check for run.py first, then run.sh
        run_script = None

        if (lesson_path / "run.py").exists():
            run_script = lesson_path / "run.py"
            command = [sys.executable, str(run_script)]
        elif (lesson_path / "run.sh").exists():
            run_script = lesson_path / "run.sh"
            command = ["bash", str(run_script)]
        else:
            return False, {"error": "No run script found (run.py or run.sh)"}

        # Execute lesson script
        self.receipt_writer.lesson_start(lesson_id, script=str(run_script))

        success, stdout, stderr = self.run_command(
            command,
            cwd=str(lesson_path),
            op_name=f"lesson_run:{lesson_id}",
            lesson_id=lesson_id
        )

        # Try to load summary.json
        summary_file = lesson_path / "summary.json"
        summary = {}

        if summary_file.exists():
            try:
                with open(summary_file, 'r') as f:
                    summary = json.load(f)
            except Exception as e:
                summary = {"error": f"Failed to load summary.json: {e}"}

        # Emit lesson_complete or lesson_skip
        if success:
            artifacts = [
                f.name
                for f in lesson_path.glob("*")
                if f.is_file() and f.name != "run.sh" and f.name != "run.py"
            ]
            self.receipt_writer.lesson_complete(lesson_id, artifacts=artifacts)
        else:
            self.receipt_writer.lesson_skip(lesson_id, reason=stderr or "Script failed")

        return success, summary


class OllamaRunner:
    """
    Execute LLM operations against Ollama.

    Handles model loading, retries, and context management.
    """

    def __init__(
        self,
        model: str = "llama2:latest",
        base_url: str = "http://localhost:11434"
    ):
        """
        Initialize Ollama runner.

        Args:
            model: Model name (e.g., "llama2:latest")
            base_url: Ollama API base URL
        """
        self.model = model
        self.base_url = base_url

    def generate(
        self,
        prompt: str,
        max_tokens: int = 1000,
        temperature: float = 0.7
    ) -> str:
        """
        Generate text using Ollama.

        Args:
            prompt: Input prompt
            max_tokens: Maximum tokens to generate
            temperature: Sampling temperature

        Returns:
            Generated text

        Note: This is a placeholder. Actual implementation would use
        ollama-python library or HTTP requests to Ollama API.
        """
        # TODO: Implement actual Ollama API call
        # Example using ollama-python:
        # import ollama
        # response = ollama.generate(model=self.model, prompt=prompt)
        # return response['response']

        # Placeholder
        return f"[Ollama response for: {prompt[:50]}...]"

    def verify_connectivity(self) -> bool:
        """
        Check if Ollama is accessible.

        Returns:
            True if Ollama is running and accessible
        """
        try:
            # TODO: Implement actual health check
            # import requests
            # response = requests.get(f"{self.base_url}/api/tags")
            # return response.status_code == 200

            # Placeholder
            return True
        except Exception:
            return False


# Example usage
if __name__ == "__main__":
    # Initialize runner
    runner = LMURunner()

    # Run a simple operation
    def example_operation():
        import time
        time.sleep(0.1)
        return {"result": "success"}

    success, result, error = runner.run_operation(
        "example_op",
        example_operation,
        lesson_id="0.1"
    )

    print(f"Operation success: {success}")
    print(f"Result: {result}")
    print(f"Error: {error}")

    # Run a command
    success, stdout, stderr = runner.run_command(
        ["echo", "Hello from LMU runner"],
        op_name="test_echo"
    )

    print(f"Command success: {success}")
    print(f"Stdout: {stdout}")

    # Test Ollama runner
    ollama = OllamaRunner()
    response = ollama.generate("What is LMU?")
    print(f"Ollama response: {response}")
