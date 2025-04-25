/**
 * This is necessary because we have transitive dependencies on CommonJS modules
 * that use require() conditionally:
 *
 * https://github.com/tapjs/signal-exit/blob/v3.0.7/index.js#L26-L27
 *
 * This is not compatible with ESM, so we need to shim require() to use the
 * CommonJS module loader.
 */
import { createRequire } from "module";
globalThis.require = createRequire(import.meta.url);
