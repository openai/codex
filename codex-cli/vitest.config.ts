import { defineConfig } from "vitest/config";

/**
 * Vitest configuration for the CLI package.
 * Disables worker threads to avoid pool recursion issues in sandbox.
 */
export default defineConfig({
  test: {
    threads: false,
    environment: "node",
  },
});
