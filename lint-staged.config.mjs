export default {
  "*.{js,jsx,ts,tsx}": ["pnpm prettier --write", "pnpm eslint --fix"],
  "*.{json,md,yml,yaml}": ["pnpm prettier --write"],
};
