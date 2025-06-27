import * as esbuild from "esbuild";
import * as fs from "fs";
import * as path from "path";

const OUT_DIR = 'dist'
/**
 * ink attempts to import react-devtools-core in an ESM-unfriendly way:
 *
 * https://github.com/vadimdemedes/ink/blob/eab6ef07d4030606530d58d3d7be8079b4fb93bb/src/reconciler.ts#L22-L45
 *
 * to make this work, we have to strip the import out of the build.
 */
const ignoreReactDevToolsPlugin = {
  name: "ignore-react-devtools",
  setup(build) {
    // When an import for 'react-devtools-core' is encountered,
    // return an empty module.
    build.onResolve({ filter: /^react-devtools-core$/ }, (args) => {
      return { path: args.path, namespace: "ignore-devtools" };
    });
    build.onLoad({ filter: /.*/, namespace: "ignore-devtools" }, () => {
      return { contents: "", loader: "js" };
    });
  },
};

// ----------------------------------------------------------------------------
// Build mode detection (production vs development)
//
//  • production (default): minified, external telemetry shebang handling.
//  • development (--dev|NODE_ENV=development|CODEX_DEV=1):
//      – no minification
//      – inline source maps for better stacktraces
//      – shebang tweaked to enable Node's source‑map support at runtime
// ----------------------------------------------------------------------------

const isDevBuild =
  process.argv.includes("--dev") ||
  process.env.CODEX_DEV === "1" ||
  process.env.NODE_ENV === "development";

const plugins = [ignoreReactDevToolsPlugin];

// Build Hygiene, ensure we drop previous dist dir and any leftover files
const outPath = path.resolve(OUT_DIR);
if (fs.existsSync(outPath)) {
  fs.rmSync(outPath, { recursive: true, force: true });
}

// Add a shebang that enables source‑map support for dev builds so that stack
// traces point to the original TypeScript lines without requiring callers to
// remember to set NODE_OPTIONS manually.
if (isDevBuild) {
  const devShebangLine =
    "#!/usr/bin/env -S NODE_OPTIONS=--enable-source-maps node\n";
  const devShebangPlugin = {
    name: "dev-shebang",
    setup(build) {
      build.onEnd(async () => {
        const outFile = path.resolve(isDevBuild ? `${OUT_DIR}/cli-dev.js` : `${OUT_DIR}/cli.js`);
        let code = await fs.promises.readFile(outFile, "utf8");
        if (code.startsWith("#!")) {
          code = code.replace(/^#!.*\n/, devShebangLine);
          await fs.promises.writeFile(outFile, code, "utf8");
        }
      });
    },
  };
  plugins.push(devShebangPlugin);
}

esbuild
  .build({
    entryPoints: ["src/cli.tsx"],
    // Do not bundle the contents of package.json at build time: always read it
    // at runtime.
    external: ["../package.json"],
    bundle: true,
    format: "esm",
    platform: "node",
    tsconfig: "tsconfig.json",
    outfile: isDevBuild ? `${OUT_DIR}/cli-dev.js` : `${OUT_DIR}/cli.js`,
    minify: !isDevBuild,
    sourcemap: isDevBuild ? "inline" : true,
    plugins,
    inject: ["./require-shim.js"],
  })
  .catch(() => process.exit(1));
