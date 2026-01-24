---
allowed-tools: Edit, Bash(git add:*), Bash(git commit:*)
description: Generate changelog for a new release version
---

You are updating the changelog for the new release.

Update CHANGELOG.md to add a new section for the new version at the top of the file, right after the '# Changelog' heading.

Review the recent commits and merged pull requests since the last release to generate meaningful changelog content for the new version. Follow the existing format in CHANGELOG.md with sections like:
- Breaking Changes (if any)
- New Features
- Bug Fixes
- Documentation
- Internal/Other changes

Include only the sections that are relevant based on the actual changes. Write clear, user-focused descriptions.

After updating CHANGELOG.md, commit the changes with the message "docs: update changelog for v{new_version}".
