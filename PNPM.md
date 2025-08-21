# Migration to pnpm (Upstream note)

Note: This document describes pnpm usage for the upstream OpenAI Codex monorepo, which includes a TypeScript package (`codex-cli`). This custom fork focuses on the Rust CLI and ships a binary named `codex-custom`. You generally do not need pnpm for `codex-custom`.

If you are maintaining upstream Node packages, the guidance below remains relevant. Otherwise, you can ignore this file when working with this fork.

This project has been migrated from npm to pnpm to improve dependency management and developer experience.

## Why pnpm?

- **Faster installation**: pnpm is significantly faster than npm and yarn
- **Disk space savings**: pnpm uses a content-addressable store to avoid duplication
- **Phantom dependency prevention**: pnpm creates a strict node_modules structure
- **Native workspaces support**: simplified monorepo management

## How to use pnpm

### Installation

```bash
# Global installation of pnpm
npm install -g pnpm@10.8.1

# Or with corepack (available with Node.js 22+)
corepack enable
corepack prepare pnpm@10.8.1 --activate
```

### Common commands

| npm command     | pnpm equivalent  |
| --------------- | ---------------- |
| `npm install`   | `pnpm install`   |
| `npm run build` | `pnpm run build` |
| `npm test`      | `pnpm test`      |
| `npm run lint`  | `pnpm run lint`  |

### Workspace-specific commands

| Action                                     | Command                                  |
| ------------------------------------------ | ---------------------------------------- |
| Run a command in a specific package        | `pnpm --filter @openai/codex run build`  |
| Install a dependency in a specific package | `pnpm --filter @openai/codex add lodash` |
| Run a command in all packages              | `pnpm -r run test`                       |

## Monorepo structure (upstream)

```
codex/
├── pnpm-workspace.yaml    # Workspace configuration
├── .npmrc                 # pnpm configuration
├── package.json           # Root dependencies and scripts
├── codex-cli/             # Upstream main TS package (not used in this fork)
│   └── package.json
└── docs/                  # Documentation (future package)
```

## Configuration files

- **pnpm-workspace.yaml**: Defines the packages included in the monorepo
- **.npmrc**: Configures pnpm behavior
- **Root package.json**: Contains shared scripts and dependencies

## CI/CD

CI/CD workflows have been updated to use pnpm instead of npm. Make sure your CI environments use pnpm 10.8.1 or higher.

## Known issues

If you encounter issues with pnpm, try the following solutions:

1. Remove the `node_modules` folder and `pnpm-lock.yaml` file, then run `pnpm install`
2. Make sure you're using pnpm 10.8.1 or higher
3. Verify that Node.js 22 or higher is installed
