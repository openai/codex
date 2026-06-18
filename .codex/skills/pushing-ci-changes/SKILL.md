---
name: pushing-ci-changes
description: Pushing GitHub Actions changes, resolving push rejection, requesting upload exceptions.
---

The Codex repository prevents anyone from uploading changes to its CI configuration unless they've
been granted a temporary role.

In order to push changes to `.github/**/*.yml` and related files you'll need the user to read
go/workflow-approvals and request an approval through that flow. It is not possible for you to
make progress by requesting an exemption yourself.

If you know you're about to push changes that would fail due to these restrictions you should still
try to push anyways to confirm the user's account doesn't already have the necessary approval.

If you encounter a failed push due to these restrictions, share the go/workflow-approvals link with
the user and ask them to return control to you once the approval has propagated to GitHub.
