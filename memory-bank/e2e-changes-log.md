# E2E & Compatibility Change Log

This file tracks all changes made specifically to enable robust E2E testing for the Ink CLI feature. Use this log to revert or clean up changes before submitting your PR, if desired.

---

## 1. tsconfig.json

- Updated `lib` to include: `["ES2015", "ES2015.Iterable", "ES2015.Promise", "ES2022", "ESNext"]` for Node/Ink/React compatibility.
- Removed `DOM` libs (not needed for CLI).

## 2. Installed Dev/Runtime Dependencies

- `node-pty` (for TTY E2E tests)
- `tsx` (for running TSX files directly, if needed)
- `typescript` (for compiling CLI entry)
- `react`, `ink`, `@types/react` (ensure available at project root)

## 3. CLI Entry

- Created/compiled `src/cli/commands/resources/list.cli.tsx` for CLI entry.
- Will point E2E tests to compiled JS output in `dist/`.

## 4. E2E Test

- Refactored E2E test to use `node-pty` for spawning CLI with a real TTY.
- Updated test to target CLI entry as needed.

## 5. (Optional) Temporary Scripts/Helpers

- Any scripts or helpers created for E2E can be removed after PR is finalized.

## 6. E2E/testing-specific patch

- After TypeScript compilation, manually patch dist/cli/commands/resources/list.cli.js:
  - Change: import ResourcesList from './list';
  - To: import ResourcesList from './list.js';
- Reason: Node ESM requires explicit .js extension for imports; TypeScript does not emit correct extension in compiled JS.
- This is a temporary step for robust E2E CLI testing and should be reverted/cleaned before merging.

## 7. Patch for agent/pagination.js import

- After TypeScript compilation, manually patch dist/cli/commands/resources/list.js:
  - Change: import { fetchPaginated, PaginationState } from '../../../agent/pagination';
  - To: import { fetchPaginated, PaginationState } from '../../../agent/pagination.js';
- Reason: Node ESM requires explicit .js extension for imports; TypeScript does not emit correct extension in compiled JS.
- This is a temporary step for robust E2E CLI testing and should be reverted/cleaned before merging.

## 8. Patch for ui/PaginatedList.js import

- After TypeScript compilation, manually patch dist/cli/commands/resources/list.js:
  - Change: import { PaginatedList } from '../../../ui/PaginatedList';
  - To: import { PaginatedList } from '../../../ui/PaginatedList.js';
- Reason: Node ESM requires explicit .js extension for imports; TypeScript does not emit correct extension in compiled JS.
- This is a temporary step for robust E2E CLI testing and should be reverted/cleaned before merging.

## 9. E2E navigation input experiments

- Tried sending 'n', 'p', and arrow keys (\x1b[C, \x1b[D) for Ink CLI navigation.
- All navigation inputs worked for page navigation in the CLI E2E harness (after increasing delay).
- Sending 'n\r', 'p\r' was not needed and did not work.
- Only the 'does not go before first page or past last page' test still fails (timeout), needs further review.
- All changes and experiments are tracked here for easy undo/cleanup.

# Steps

1. Compile with: npx tsc --project tsconfig.json
2. Patch with: sed -i '' "s|from './list'|from './list.js'|" dist/cli/commands/resources/list.cli.js
3. Patch with: sed -i '' "s|from '../../../agent/pagination'|from '../../../agent/pagination.js'|" dist/cli/commands/resources/list.js
4. Patch with: sed -i '' "s|from '../../../ui/PaginatedList'|from '../../../ui/PaginatedList.js'|" dist/cli/commands/resources/list.js
5. Run E2E: npx vitest run src/cli/commands/resources/list.e2e.test.ts
6. Navigation tests: try 'n', 'p', and arrow keys (\x1b[C, \x1b[D) as inputs. Remove 'n\r', 'p\r' from test suite.

---

**Review this file before submitting your PR to decide which changes to keep or revert!**
