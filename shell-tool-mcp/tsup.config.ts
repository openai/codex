import { defineConfig } from "tsup";

export default defineConfig({
  entry: {
    "mcp-server": "src/index.ts",
  },
  outDir: "bin",
  format: ["cjs"],
  target: "node18",
  clean: true,
  sourcemap: false,
  banner: {
    js: "#!/usr/bin/env node",
  },
});
