# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands
- Build: `cd codex-cli && npm run build`
- Lint: `cd codex-cli && npm run lint`
- Format: `cd codex-cli && npm run format`
- Typecheck: `cd codex-cli && npm run typecheck`
- Test: `cd codex-cli && npm test`
- Test (watch mode): `cd codex-cli && npm run test:watch`
- Test single file: `cd codex-cli && npx vitest run tests/[test-file].test.ts`

## Style Guidelines
- Use TypeScript with strict type checking
- Follow ESLint and Prettier configurations
- Keep functions small and focused on a single responsibility
- Use async/await for asynchronous code
- Use React Hooks for all React components
- Follow existing naming conventions (camelCase for variables/functions, PascalCase for components/classes)
- Always handle errors appropriately; prefer explicit error handling over try/catch when possible
- Maintain 100% test coverage for new features
- Respect existing directory structure and module organization
- Keep imports organized (React imports first, then external libraries, then internal modules)
- Commits should have atomicity - each commit should compile and pass tests