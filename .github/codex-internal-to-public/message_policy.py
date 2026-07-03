#!/usr/bin/env python3

"""Define the trust boundary for public Codex export commit messages.

The internal-to-public workflow must produce useful public metadata without
giving the model the raw internal commit message or blindly publishing model
or operator output. This module exposes only mechanically allowlisted
``openai/codex`` references from the source message, bounds reference artifacts,
and validates the resulting subject and body before a write-capable job can
publish them.

Source text, extracted references, model output, and manual overrides are all
untrusted. The checks here reject malformed output, known internal references,
private document URLs, unsupported GitHub references, and unverifiable public
links. These checks are a mechanical backstop; callers must still restrict
model inputs to the public projection.
"""

import re
import shlex
import subprocess
import sys
from pathlib import Path

MAX_COMMIT_BODY_BYTES = 5_000
MAX_PUBLIC_REFERENCES = 10
MAX_PUBLIC_REFERENCES_BYTES = 2_000
PUBLIC_URL_RE = re.compile(
    r"https://github\.com/openai/codex/(?:pull/[0-9]{1,10}|commit/[0-9a-fA-F]{40})"
)
PR_URL_RE = re.compile(r"https://github\.com/openai/codex/pull/([0-9]{1,10})")
COMMIT_URL_RE = re.compile(r"https://github\.com/openai/codex/commit/([0-9a-fA-F]{40})")
BARE_COMMIT_RE = re.compile(r"(?<![0-9a-fA-F])[0-9a-fA-F]{7,40}(?![0-9a-fA-F])")
FORBIDDEN_MESSAGE_PATTERNS = {
    "an internal Codex repository reference": re.compile(
        r"codex-internal|github\.com/openai/codex-internal", re.IGNORECASE
    ),
    "an internal synchronization implementation detail": re.compile(
        r"copybara|copyberry|copybara-no-internal-(?:code|references)|\.github/internal",
        re.IGNORECASE,
    ),
    "a Slack URL": re.compile(r"https?://[^\s)>]*slack\.com", re.IGNORECASE),
    "a Notion URL": re.compile(
        r"https?://[^\s)>]*(?:notion\.so|notion\.site)", re.IGNORECASE
    ),
    "a Google Docs or Drive URL": re.compile(
        r"https?://(?:docs|drive)\.google\.com", re.IGNORECASE
    ),
    "a shorthand GitHub reference": re.compile(
        r"(?:openai/codex)?#[0-9]+", re.IGNORECASE
    ),
}


def validate_message(
    message: str,
    public_repo: Path,
    output_file: Path,
    *,
    verify_public_references: bool = True,
) -> None:
    if "\r" in message:
        raise RuntimeError("Commit message must use Unix line endings.")
    message = message.rstrip()
    if message.lstrip().startswith("{") and message.endswith("}"):
        raise RuntimeError("Codex output must be the Markdown commit message, not JSON.")
    subject, separator, body = message.partition("\n")
    subject = subject.strip()
    if not subject:
        raise RuntimeError("Commit subject must be non-empty.")
    if separator and not body.startswith("\n"):
        raise RuntimeError("Commit body must be separated from the subject by a blank line.")
    body = body[1:].strip() if separator else ""
    body_bytes = len(body.encode("utf-8"))
    if body_bytes > MAX_COMMIT_BODY_BYTES:
        raise RuntimeError(
            f"Commit body is {body_bytes} bytes; the limit is {MAX_COMMIT_BODY_BYTES}."
        )

    message = subject
    if body:
        message = f"{message}\n\n{body}"
    validate_message_text(
        message,
        public_repo,
        verify_public_references=verify_public_references,
    )
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(f"{message}\n", encoding="utf-8")


def render_public_references(source_message: str) -> str:
    references = sorted(set(PUBLIC_URL_RE.findall(source_message)))
    if len(references) > MAX_PUBLIC_REFERENCES:
        raise RuntimeError(
            f"Source message contains {len(references)} public references; "
            f"the limit is {MAX_PUBLIC_REFERENCES}."
        )

    lines = [
        "# Candidate public references",
        "",
        "These URLs were extracted mechanically from the source commit message.",
        "Use one only when it is relevant to the public diff and can be verified.",
        "",
    ]
    lines.extend(f"- {reference}" for reference in references)
    if not references:
        lines.append("No public Codex PR or commit URLs were found.")
    rendered = "\n".join(lines) + "\n"
    ensure_model_input_size(
        "public reference context", rendered, MAX_PUBLIC_REFERENCES_BYTES
    )
    return rendered


def ensure_model_input_size(label: str, value: str, max_bytes: int) -> None:
    size = len(value.encode("utf-8"))
    if size > max_bytes:
        raise RuntimeError(f"{label} is {size} bytes; the limit is {max_bytes} bytes.")


def validate_message_text(
    message: str, public_repo: Path, *, verify_public_references: bool
) -> None:
    for description, pattern in FORBIDDEN_MESSAGE_PATTERNS.items():
        match = pattern.search(message)
        if match:
            raise RuntimeError(
                f"Generated commit message contains {description}: {match.group(0)}"
            )

    for match in re.finditer(r"https://github\.com/openai/[^\s)>]+", message):
        if not PUBLIC_URL_RE.fullmatch(match.group(0).rstrip(".,")):
            raise RuntimeError(
                "Generated commit message contains an unsupported OpenAI GitHub URL: "
                f"{match.group(0)}"
            )

    message_without_public_urls = PUBLIC_URL_RE.sub("", message)
    bare_commit = BARE_COMMIT_RE.search(message_without_public_urls)
    if bare_commit:
        raise RuntimeError(
            "Generated commit message contains a bare or abbreviated commit SHA: "
            f"{bare_commit.group(0)}"
        )

    if verify_public_references:
        for pr_number in PR_URL_RE.findall(message):
            run(["gh", "api", f"repos/openai/codex/pulls/{pr_number}"], capture=True)
        for commit_revision in COMMIT_URL_RE.findall(message):
            run(
                ["gh", "api", f"repos/openai/codex/commits/{commit_revision}"],
                capture=True,
            )
    else:
        for commit_revision in COMMIT_URL_RE.findall(message):
            run(
                ["git", "cat-file", "-e", f"{commit_revision}^{{commit}}"],
                cwd=public_repo,
            )


def run(
    args: list[str],
    *,
    cwd: Path | None = None,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    print(f"+ {shlex.join(args)}", flush=True)
    completed = subprocess.run(
        args,
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if completed.returncode != 0:
        detail = f"\n{completed.stdout}" if capture and completed.stdout else ""
        raise RuntimeError(
            f"Command failed with exit code {completed.returncode}: "
            f"{shlex.join(args)}{detail}"
        )
    return completed


def main() -> int:
    if len(sys.argv) != 4 or sys.argv[1] not in {"check", "check-offline"}:
        raise RuntimeError(
            "usage: message_policy.py check|check-offline "
            "<public-repo> <message-file>"
        )
    command = sys.argv[1]
    message_file = Path(sys.argv[3])
    validate_message(
        message_file.read_text(encoding="utf-8"),
        Path(sys.argv[2]),
        message_file,
        verify_public_references=command == "check",
    )
    print("Commit message satisfies the public message policy.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
