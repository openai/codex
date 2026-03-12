Inspect the current project before asking for any permissions.

Start by using non-mutating discovery tools to understand how this project is run, built, tested, or set up. Read the most relevant project files first, such as README files, manifest files, lockfiles, task runners, devcontainer or Docker files, and any repo-local instructions that indicate required tools, services, directories, or network access.

After you have concrete evidence from the repo, call the `request_permissions` tool with the narrowest permission profile that would help you run or set up this project effectively. Include a short reason that references the task you are trying to unblock.

Be strict about scope:
- Request only permissions that are justified by the project contents you found.
- Prefer the smallest possible filesystem read/write path list.
- Request network access only if the repo indicates it is needed for dependency installation, fetching services, or other project setup steps.
- Request macOS permissions only if the repo clearly implies they are needed.
- Do not ask for broad or speculative permissions.
- Do not ask for permissions that the current workspace access already provides.

Once the permission request is made, briefly explain what you inspected and why those permissions were selected.
