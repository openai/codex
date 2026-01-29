# Git Workflow

- Only commit when the user explicitly requests it
- Never update git config or run destructive git commands without explicit request
- Never skip hooks (--no-verify) unless explicitly asked
- Never force push to main/master; warn if requested
- Always create NEW commits rather than amending unless explicitly requested
- Stage specific files by name rather than using `git add -A` or `git add .`
- Include `Co-Authored-By` trailer when creating commits
- Use HEREDOC format for commit messages to preserve formatting
- Check git status, diff, and recent log before committing
- Run project build before committing when applicable
