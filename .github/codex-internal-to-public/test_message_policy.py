import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import call, patch

import message_policy


class ValidateMessageTest(unittest.TestCase):
    def test_writes_valid_message_and_verifies_references(self) -> None:
        commit = "0123456789abcdef0123456789abcdef01234567"
        value = (
            "Improve public behavior\n\n"
            "See https://github.com/openai/codex/pull/12345 and "
            f"https://github.com/openai/codex/commit/{commit}."
        )
        with (
            tempfile.TemporaryDirectory() as temp_dir,
            patch.object(message_policy, "run") as run,
        ):
            root = Path(temp_dir)
            output = root / "message.txt"
            message_policy.validate_message(value, root, output)
            actual = output.read_text(encoding="utf-8")

        self.assertEqual(
            actual,
            "Improve public behavior\n\n"
            "See https://github.com/openai/codex/pull/12345 and "
            f"https://github.com/openai/codex/commit/{commit}.\n",
        )
        self.assertEqual(
            run.call_args_list,
            [
                call(
                    ["gh", "api", "repos/openai/codex/pulls/12345"],
                    capture=True,
                ),
                call(
                    ["git", "cat-file", "-e", f"{commit}^{{commit}}"],
                    cwd=root,
                ),
            ],
        )

    def test_rejects_shorthand_github_reference(self) -> None:
        value = "Improve public behavior\n\nSee openai/codex#12345."
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(RuntimeError, "shorthand GitHub reference"):
                message_policy.validate_message(
                    value, Path(temp_dir), Path(temp_dir) / "out"
                )

    def test_rejects_bare_commit_sha(self) -> None:
        value = "Improve behavior\n\nOriginally fixed in deadbeef."
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(RuntimeError, "bare or abbreviated commit SHA"):
                message_policy.validate_message(
                    value, Path(temp_dir), Path(temp_dir) / "out"
                )

    def test_rejects_sensitive_urls(self) -> None:
        urls = [
            "https://openai.slack.com/archives/C123/p456",
            "https://www.notion.so/private-page",
            "https://docs.google.com/document/d/private",
            "https://drive.google.com/file/d/private",
        ]
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for url in urls:
                with self.subTest(url=url), self.assertRaises(RuntimeError):
                    message_policy.validate_message(
                        f"Improve behavior\n\n{url}",
                        root,
                        root / "out",
                    )

    def test_rejects_unsupported_openai_github_url(self) -> None:
        value = (
            "Improve public behavior\n\n"
            "See https://github.com/openai/private-repo/pull/123."
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(RuntimeError, "unsupported OpenAI GitHub URL"):
                message_policy.validate_message(
                    value, Path(temp_dir), Path(temp_dir) / "out"
                )

    def test_rejects_body_without_blank_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(RuntimeError, "separated.*blank line"):
                message_policy.validate_message(
                    "Improve behavior\nBody without a separator.",
                    Path(temp_dir),
                    Path(temp_dir) / "out",
                )

    def test_rejects_json_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(RuntimeError, "Markdown.*not JSON"):
                message_policy.validate_message(
                    '{"subject": "Improve behavior", "body": ""}',
                    Path(temp_dir),
                    Path(temp_dir) / "out",
                )

    def test_allows_long_subject_and_fenced_markdown_body(self) -> None:
        subject = (
            "Explain the complete public behavior even when the subject exceeds "
            "the traditional Git guideline"
        )
        message = (
            f"{subject}\n\n"
            "## Example\n\n"
            "```rust\n"
            "fn main() {\n"
            '    println!("ready");\n'
            "}\n"
            "```"
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "message.md"
            message_policy.validate_message(message, Path(temp_dir), output)

            self.assertEqual(output.read_text(encoding="utf-8"), f"{message}\n")

    def test_rejects_body_larger_than_byte_limit(self) -> None:
        body = "é" * (message_policy.MAX_COMMIT_BODY_BYTES // 2 + 1)
        body_bytes = len(body.encode("utf-8"))
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaisesRegex(
                RuntimeError,
                f"{body_bytes} bytes; the limit is {message_policy.MAX_COMMIT_BODY_BYTES}",
            ):
                message_policy.validate_message(
                    f"Explain public behavior\n\n{body}",
                    Path(temp_dir),
                    Path(temp_dir) / "out",
                )

    def test_offline_cli_checks_message_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            scratch = Path(temp_dir) / "scratch"
            scratch.mkdir()
            draft = scratch / "draft.md"
            draft.write_text(
                "Explain public behavior\n\n## What changed\n\nDetails.\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    sys.executable,
                    Path(message_policy.__file__).resolve(),
                    "check-offline",
                    temp_dir,
                    draft,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                result.stdout, "Commit message satisfies the public message policy.\n"
            )
            self.assertEqual(
                draft.read_text(encoding="utf-8"),
                "Explain public behavior\n\n## What changed\n\nDetails.\n",
            )


class PublicReferenceContextTest(unittest.TestCase):
    def test_rejects_too_many_references(self) -> None:
        references = "\n".join(
            f"https://github.com/openai/codex/pull/{number}"
            for number in range(message_policy.MAX_PUBLIC_REFERENCES + 1)
        )

        with self.assertRaisesRegex(RuntimeError, "the limit is 10"):
            message_policy.render_public_references(references)


if __name__ == "__main__":
    unittest.main()
