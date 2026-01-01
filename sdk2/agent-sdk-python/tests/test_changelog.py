import re
from pathlib import Path


class TestChangelog:
    def setup_method(self):
        self.changelog_path = Path(__file__).parent.parent / "CHANGELOG.md"

    def test_changelog_exists(self):
        assert self.changelog_path.exists(), "CHANGELOG.md file should exist"

    def test_changelog_starts_with_header(self):
        content = self.changelog_path.read_text()
        assert content.startswith("# Changelog"), (
            "Changelog should start with '# Changelog'"
        )

    def test_changelog_has_valid_version_format(self):
        content = self.changelog_path.read_text()
        lines = content.split("\n")

        version_pattern = re.compile(r"^## \d+\.\d+\.\d+(?:\s+\(\d{4}-\d{2}-\d{2}\))?$")
        versions = []

        for line in lines:
            if line.startswith("## "):
                assert version_pattern.match(line), f"Invalid version format: {line}"
                version_match = re.match(r"^## (\d+\.\d+\.\d+)", line)
                if version_match:
                    versions.append(version_match.group(1))

        assert len(versions) > 0, "Changelog should contain at least one version"

    def test_changelog_has_bullet_points(self):
        content = self.changelog_path.read_text()
        lines = content.split("\n")

        in_version_section = False
        has_bullet_points = False

        for i, line in enumerate(lines):
            if line.startswith("## "):
                if in_version_section and not has_bullet_points:
                    raise AssertionError(
                        "Previous version section should have at least one bullet point"
                    )
                in_version_section = True
                has_bullet_points = False
            elif in_version_section and line.startswith("- "):
                has_bullet_points = True
            elif in_version_section and line.strip() == "" and i == len(lines) - 1:
                # Last line check
                assert has_bullet_points, (
                    "Each version should have at least one bullet point"
                )

        # Check the last section
        if in_version_section:
            assert has_bullet_points, (
                "Last version section should have at least one bullet point"
            )

    def test_changelog_versions_in_descending_order(self):
        content = self.changelog_path.read_text()
        lines = content.split("\n")

        versions = []
        for line in lines:
            if line.startswith("## "):
                version_match = re.match(r"^## (\d+)\.(\d+)\.(\d+)", line)
                if version_match:
                    versions.append(tuple(map(int, version_match.groups())))

        for i in range(1, len(versions)):
            assert versions[i - 1] > versions[i], (
                f"Versions should be in descending order: {versions[i - 1]} should be > {versions[i]}"
            )

    def test_changelog_no_empty_bullet_points(self):
        content = self.changelog_path.read_text()
        lines = content.split("\n")

        for line in lines:
            if line.strip() == "-":
                raise AssertionError("Changelog should not have empty bullet points")
