# To do list with go here
Top to do:
- Scroll to top
- 

New features
- render an image preview in ANSI space

## Running it
Using pnpm (recommended):

```bash
pnpm install
pnpm build
env -u OPENAI_PROJECT -u OPENAI_ORGANIZATION node ./bin/codex.js
```

Dev build that runs immediately:

```bash
pnpm build:dev
```

Or with npm:

```bash
npm install
npm run build
env -u OPENAI_PROJECT -u OPENAI_ORGANIZATION node ./bin/codex.js
```
