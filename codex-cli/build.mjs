import * as esbuild from "esbuild";
import * as fs from "fs";
import * as path from "path";
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
        const outFile = path.resolve(isDevBuild ? "dist/cli-dev.js" : "dist/cli.js");
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

// Add a plugin to handle native modules properly
const nativeModulesPlugin = {
  name: "native-modules-handler",
  setup(build) {
    // Handle .node files with the copy loader
    build.onResolve({ filter: /\.node$/ }, (args) => {
      return { path: args.path, external: true };
    });

    // Mark keytar as external to prevent bundling issues
    build.onResolve({ filter: /^keytar$/ }, (args) => {
      return { path: args.path, external: true };
    });
  },
};

// Add the native modules plugin to the plugins array
plugins.push(nativeModulesPlugin);

esbuild
  .build({
    entryPoints: ["src/cli.tsx"],
    bundle: true,
    format: "esm",
    platform: "node",
    tsconfig: "tsconfig.json",
    outfile: isDevBuild ? "dist/cli-dev.js" : "dist/cli.js",
    minify: !isDevBuild,
    sourcemap: isDevBuild ? "inline" : true,
    plugins,
    inject: ["./require-shim.js"],
    // Set native modules as external
    external: ["keytar"],
    // Configure loader for .node files
    loader: { ".node": "file" },
  })
  .catch(() => process.exit(1));
