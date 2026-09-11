"""Offline checks for the public Windows build-input bootstrap."""

import hashlib
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("voice-cygwin-inputs.py")
SPEC = importlib.util.spec_from_file_location("voice_cygwin_inputs", SCRIPT)
inputs = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inputs)


class VoiceCygwinInputsTests(unittest.TestCase):
    def test_input_paths_cannot_escape_the_local_cache(self):
        manifest = {
            "metadata": [
                {"file": "x86_64/setup.xz"},
                {"file": "x86_64/setup.xz.sig"},
            ],
            "packages": [{"file": "../outside.tar.xz"}],
        }
        with self.assertRaisesRegex(ValueError, "unsafe Cygwin input"):
            inputs.records(manifest)

    def test_extract_verifies_archive_and_every_input_before_offline_installation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payloads = {
                "x86_64/setup.xz": b"metadata",
                "x86_64/setup.xz.sig": b"signature",
                "x86_64/release/bash/bash.tar.xz": b"package",
            }
            records = [
                {
                    "file": name,
                    "bytes": len(data),
                    "sha512": hashlib.sha512(data).hexdigest(),
                }
                for name, data in payloads.items()
            ]
            archive = root / "inputs.tar.gz"

            def write_archive(contents):
                with tarfile.open(archive, "w:gz") as bundle:
                    for name, data in contents.items():
                        info = tarfile.TarInfo(name)
                        info.size = len(data)
                        bundle.addfile(info, io.BytesIO(data))
                return {
                    "bytes": archive.stat().st_size,
                    "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                }

            manifest = {
                "metadata": records[:2],
                "packages": records[2:],
                "archive": write_archive(payloads),
            }
            destination = root / "cache"
            inputs.extract(manifest, archive, destination)
            self.assertEqual(
                {
                    record["file"]: (destination / record["file"]).read_bytes()
                    for record in records
                },
                payloads,
            )

            (destination / records[2]["file"]).unlink()
            payloads[records[2]["file"]] = b"wrong"
            manifest["archive"] = write_archive(payloads)
            with self.assertRaisesRegex(ValueError, "Cygwin input mismatch"):
                inputs.extract(manifest, archive, root / "rejected")
            self.assertFalse((root / "rejected").exists())


if __name__ == "__main__":
    unittest.main()
