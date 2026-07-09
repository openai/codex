import runpy
import unittest
from pathlib import Path
from unittest.mock import Mock
from unittest.mock import patch


MODULE = runpy.run_path(Path(__file__).with_name("copybara_public_to_internal.py"))
PublicChange = MODULE["PublicChange"]
GitAuthor = MODULE["GitAuthor"]
open_or_update_pr = MODULE["open_or_update_pr"]


class OpenOrUpdatePrTest(unittest.TestCase):
    def test_replaces_tree_empty_sync_commit_with_marker(self) -> None:
        change = PublicChange(
            rev="a" * 40,
            author=GitAuthor(
                name="Public Author",
                email="public@example.com",
                date="2026-07-09T00:00:00Z",
            ),
            title="Public change",
            body="",
            url=None,
            number=None,
        )
        body_file = Path("body.md")
        message_file = Path("message.md")
        mocks = {
            "run": Mock(),
            "fetch_sync_branch": Mock(),
            "trees_match": Mock(return_value=True),
            "create_empty_import_marker_commit": Mock(),
            "output": Mock(return_value="1"),
            "validate_sync_branch_paths": Mock(),
            "find_open_pr": Mock(side_effect=[None, "1256"]),
        }

        with patch.dict(open_or_update_pr.__globals__, mocks):
            pr_number = open_or_update_pr(change, body_file, message_file)

        self.assertEqual(pr_number, "1256")
        mocks["create_empty_import_marker_commit"].assert_called_once_with(
            change, message_file
        )
        self.assertEqual(mocks["fetch_sync_branch"].call_count, 2)
        mocks["validate_sync_branch_paths"].assert_called_once_with(change.rev)


if __name__ == "__main__":
    unittest.main()
