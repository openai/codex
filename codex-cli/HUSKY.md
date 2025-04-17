# Husky Git Hooks

This project uses [Husky](https://typicode.github.io/husky/) to enforce code quality checks before commits and pushes.

## What's Included

- **Pre-commit Hook**: Runs lint-staged to check files that are about to be committed.

  - Lints and formats TypeScript/TSX files using ESLint and Prettier
  - Formats JSON, MD, and YML files using Prettier

- **Pre-push Hook**: Runs tests and type checking before pushing to the remote repository.
  - Executes `npm test` to run all tests
  - Executes `npm run typecheck` to check TypeScript types

## Benefits

- Ensures consistent code style across the project
- Prevents pushing code with failing tests or type errors
- Reduces the need for style-related code review comments
- Improves overall code quality

## For Contributors

You don't need to do anything special to use these hooks. They will automatically run when you commit or push code.

If you need to bypass the hooks in exceptional cases:

```bash
# Skip pre-commit hooks
git commit -m "Your message" --no-verify

# Skip pre-push hooks
git push --no-verify
```

Note: Please use these bypass options sparingly and only when absolutely necessary.

## Troubleshooting

If you encounter any issues with the hooks:

1. Make sure you have the latest dependencies installed: `npm install`
2. Ensure the hook scripts are executable (Unix systems): `chmod +x .husky/pre-commit .husky/pre-push`
3. Check if there are any ESLint or Prettier configuration issues in your code
