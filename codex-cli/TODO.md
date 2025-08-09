# To do list with go here
Top to do (sm)
speed up animation
group read together
remove space between > and assistant message

images
- https://github.com/chjj/tng
- https://github.com/chjj/blessed
- https://github.com/atanunq/viu

- fix slash command formatting
- improve @ commands

New features
- render an image preview in ANSI space


js```import OpenAI from "openai";
const openai = new OpenAI();

const response = await openai.responses.create({
  model: "gpt-5",
  input: "How much gold would it take to coat the Statue of Liberty in a 1mm layer?",
  reasoning: {
    effort: "minimal" // "minimal", "medium", "high"
  }
  text: {
    verbosity: "low" // "low", "medium", "high"
  }
});

console.log(response);``

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
