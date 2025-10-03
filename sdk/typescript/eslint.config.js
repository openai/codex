import eslint from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import nodeImport from "eslint-plugin-node-import";

export default defineConfig(eslint.configs.recommended, tseslint.configs.recommended, {
  plugins: {
    "node-import": nodeImport,
  },

  rules: {
    "node-import/prefer-node-protocol": 2,
  },
});
