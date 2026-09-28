import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from select_windows_bazel_targets import DURATIONS_PATH
from select_windows_bazel_targets import assign_targets
from select_windows_bazel_targets import read_durations


class SelectWindowsBazelTargetsTest(unittest.TestCase):
    def test_balances_targets_with_complete_deterministic_coverage(self) -> None:
        durations = {
            f"//test:{name}": duration
            for name, duration in zip("abcdefgh", range(8, 0, -1))
        }
        targets = list(durations)

        assignments, loads = assign_targets(targets, durations, 4)

        self.assertEqual(loads, [9, 9, 9, 9])
        self.assertEqual(
            sorted(target for shard in assignments for target in shard), sorted(targets)
        )
        self.assertEqual(
            assign_targets(list(reversed(targets)), durations, 4), (assignments, loads)
        )
        self.assertTrue(all(shard == sorted(shard) for shard in assignments))

    def test_missing_weights_use_one_and_unused_weights_do_not_create_targets(
        self,
    ) -> None:
        targets = ["//test:c", "//test:a", "//test:b"]

        assignments, loads = assign_targets(targets, {"//test:stale": 100}, 2)

        self.assertEqual(assignments, [["//test:a", "//test:c"], ["//test:b"]])
        self.assertEqual(loads, [2, 1])

    def test_rejects_empty_or_duplicate_target_input(self) -> None:
        for targets in ([], [""], ["//test:a", "//test:a"]):
            with self.subTest(targets=targets), self.assertRaises(ValueError):
                assign_targets(targets, {}, 4)

    def test_reads_duration_file_and_rejects_invalid_weights(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "durations.tsv"
            path.write_text(
                "# comment\n\n//test:a\t12\n//test:b\t1\n", encoding="utf-8"
            )
            self.assertEqual(read_durations(path), {"//test:a": 12, "//test:b": 1})

            for contents in (
                "//test:a 12\n",
                "//test:a\t0\n",
                "//test:a\t-1\n",
                "//test:a\t1.5\n",
                "//test:a\t1\n//test:a\t2\n",
            ):
                with self.subTest(contents=contents), self.assertRaises(ValueError):
                    path.write_text(contents, encoding="utf-8")
                    read_durations(path)

    def test_cli_uses_checked_in_weights_and_writes_only_lf_delimited_targets(
        self,
    ) -> None:
        durations = read_durations(DURATIONS_PATH)
        known = sorted(durations, key=lambda target: (-durations[target], target))[:4]
        self.assertEqual(len(known), 4)
        targets = [*known, "//test:new-target"]
        expected, _ = assign_targets(targets, durations, 4)
        script = DURATIONS_PATH.with_name("select_windows_bazel_targets.py")

        for shard, assignment in enumerate(expected, 1):
            command = [
                sys.executable,
                str(script),
                "--shard",
                str(shard),
                "--shard-count",
                "4",
            ]
            for ordered_targets in (targets, list(reversed(targets))):
                with self.subTest(shard=shard, ordered_targets=ordered_targets):
                    result = subprocess.run(
                        command,
                        input=("\r\n".join(ordered_targets) + "\r\n").encode(),
                        capture_output=True,
                        check=True,
                    )
                    self.assertEqual(
                        result.stdout, ("\n".join(assignment) + "\n").encode()
                    )
                    self.assertIn(b"default weights=1", result.stderr)

    def test_cli_uses_only_the_requested_duration_file(self) -> None:
        script = DURATIONS_PATH.with_name("select_windows_bazel_targets.py").resolve()
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "durations.tsv"
            path.write_text("//test:a\t1\n//test:b\t9\n", encoding="utf-8")
            command = [sys.executable, str(script), "--shard-count", "2"]
            for shard, expected in ((1, b"//test:b\n"), (2, b"//test:a\n//test:c\n")):
                with self.subTest(shard=shard):
                    result = subprocess.run(
                        [*command, "--shard", str(shard), "--durations", path.name],
                        input=b"//test:a\n//test:b\n//test:c\n",
                        cwd=temp_dir,
                        capture_output=True,
                        check=True,
                    )
                    self.assertEqual(result.stdout, expected)
                    self.assertIn(b"estimated test-seconds=[9, 2]", result.stderr)
                    self.assertIn(b"unused weights=0", result.stderr)

            result = subprocess.run(
                [*command, "--shard", "1", "--durations", "missing.tsv"],
                input=b"//test:a\n",
                cwd=temp_dir,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
            self.assertIn(b"missing.tsv", result.stderr)

    def test_cli_rejects_bad_or_empty_selection_without_partial_output(self) -> None:
        script = DURATIONS_PATH.with_name("select_windows_bazel_targets.py")
        for targets, error in (
            (b"", b"no Bazel test targets were provided"),
            (b"//test:a\n//test:a\n", b"duplicate Bazel test target labels"),
            (b"//test:a\n", b"no Bazel test targets selected for shard 2/2"),
        ):
            with self.subTest(targets=targets):
                result = subprocess.run(
                    [sys.executable, str(script), "--shard", "2", "--shard-count", "2"],
                    input=targets,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertIn(error, result.stderr)


if __name__ == "__main__":
    unittest.main()
