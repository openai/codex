import plistlib
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SIGNING_DIR = ROOT / ".github" / "scripts" / "macos-signing"
SELECTOR = SIGNING_DIR / "select_codex_entitlements.sh"
DEFAULT_ENTITLEMENTS = SIGNING_DIR / "codex.entitlements.plist"
INTEL_ENTITLEMENTS = (
    SIGNING_DIR / "codex-x86_64-apple-darwin.entitlements.plist"
)


class MacosSigningEntitlementsTest(unittest.TestCase):
    def select(self, target: str, binary: str) -> Path:
        result = subprocess.run(
            ["bash", str(SELECTOR), target, binary],
            check=True,
            capture_output=True,
            text=True,
        )
        return Path(result.stdout.strip())

    def test_intel_v8_binaries_allow_unsigned_executable_memory(self) -> None:
        for binary in ["codex", "codex-app-server", "codex-code-mode-host"]:
            with self.subTest(binary=binary):
                self.assertEqual(
                    self.select("x86_64-apple-darwin", binary),
                    INTEL_ENTITLEMENTS,
                )

    def test_other_macos_release_binaries_keep_existing_entitlements(self) -> None:
        target_binaries = [
            ("aarch64-apple-darwin", "codex"),
            ("aarch64-apple-darwin", "codex-app-server"),
            ("aarch64-apple-darwin", "codex-code-mode-host"),
            ("aarch64-apple-darwin", "codex-responses-api-proxy"),
            ("x86_64-apple-darwin", "codex-responses-api-proxy"),
        ]
        for target, binary in target_binaries:
            with self.subTest(target=target, binary=binary):
                self.assertEqual(self.select(target, binary), DEFAULT_ENTITLEMENTS)

    def test_selector_fails_closed_for_unknown_inputs(self) -> None:
        for target, binary in [
            ("unknown-apple-darwin", "codex"),
            ("x86_64-apple-darwin", "unknown"),
        ]:
            with self.subTest(target=target, binary=binary):
                result = subprocess.run(
                    ["bash", str(SELECTOR), target, binary],
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(result.returncode, 2)

    def test_entitlement_profiles_are_least_privilege(self) -> None:
        with DEFAULT_ENTITLEMENTS.open("rb") as file:
            default = plistlib.load(file)
        with INTEL_ENTITLEMENTS.open("rb") as file:
            intel = plistlib.load(file)

        self.assertEqual(default, {"com.apple.security.cs.allow-jit": True})
        self.assertEqual(
            intel,
            {
                "com.apple.security.cs.allow-jit": True,
                "com.apple.security.cs.allow-unsigned-executable-memory": True,
            },
        )


if __name__ == "__main__":
    unittest.main()
