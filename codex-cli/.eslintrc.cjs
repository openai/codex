module.exports = {
  root: true,
  env: { browser: true, node: true, es2020: true },
  extends: [
    "eslint:recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:react-hooks/recommended",
  ],
  ignorePatterns: [
    ".eslintrc.cjs",
    "build.mjs",
    "dist",
    "vite.config.ts",
    "src/components/vendor",
  ],
  parser: "@typescript-eslint/parser",
  parserOptions: {
    tsconfigRootDir: __dirname,
    project: ["./tsconfig.json"],
  },
  plugins: ["import", "react-hooks", "react-refresh"],
  rules: {
    // Imports
    "@typescript-eslint/consistent-type-imports": "error",
    "import/no-cycle": ["error", { maxDepth: 1 }],
    "import/no-duplicates": "error",
    "import/order": [
      "error",
      {
        groups: ["type"],
        "newlines-between": "always",
        alphabetize: {
          order: "asc",
          caseInsensitive: false,
        },
      },
    ],
    // We use the import/ plugin instead.
    "sort-imports": "off",

    "@typescript-eslint/array-type": ["error", { default: "generic" }],
    // FIXME(mbolin): Introduce this.
    // "@typescript-eslint/explicit-function-return-type": "error",
    "@typescript-eslint/explicit-module-boundary-types": "error",
    "@typescript-eslint/no-explicit-any": "error",
    "@typescript-eslint/switch-exhaustiveness-check": [
      "error",
      {
        allowDefaultCaseForExhaustiveSwitch: false,
        requireDefaultForNonUnion: true,
      },
    ],

    // Use typescript-eslint/no-unused-vars, no-unused-vars reports
    // false positives with typescript
    "no-unused-vars": "off",
    "@typescript-eslint/no-unused-vars": [
      "error",
      {
        argsIgnorePattern: "^_",
        varsIgnorePattern: "^_",
        caughtErrorsIgnorePattern: "^_",
      },
    ],

    curly: "error",

    eqeqeq: ["error", "always", { null: "never" }],
    "react-refresh/only-export-components": [
      "error",
      { allowConstantExport: true },
    ],
    "no-await-in-loop": "error",
    "no-bitwise": "error",
    "no-caller": "error",
    // This is fine during development, but should not be checked in.
    "no-console": "error",
    // This is fine during development, but should not be checked in.
    "no-debugger": "error",
    "no-duplicate-case": "error",
    "no-eval": "error",
    "no-ex-assign": "error",
    "no-return-await": "error",
    "no-param-reassign": "error",
    "no-script-url": "error",
    "no-self-compare": "error",
    "no-unsafe-finally": "error",
    "no-var": "error",
    "react-hooks/rules-of-hooks": "error",
    "react-hooks/exhaustive-deps": "error",
  },
  overrides: [
    {
      // apply only to files under tests/
      files: ["tests/**/*.{ts,tsx,js,jsx}"],
      rules: {
        "@typescript-eslint/no-explicit-any": "off",
        "import/order": "off",
        "@typescript-eslint/explicit-module-boundary-types": "off",
        "@typescript-eslint/ban-ts-comment": "off",
        "@typescript-eslint/no-var-requires": "off",
        "no-await-in-loop": "off",
        "no-control-regex": "off",
      },
    },
  ],
};
