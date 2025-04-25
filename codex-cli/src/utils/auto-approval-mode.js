// This tiny shim exists solely so that development tooling such as `ts-node`
// (which executes the *source* files directly) can resolve the existing
// `./auto-approval-mode.js` import specifier used throughout the code‑base.
//
// In the emitted JavaScript (built via `tsc --module nodenext`) the compiler
// rewrites the path to point at the generated `.js` file automatically, so
// having this shim in the source tree is completely transparent for
// production builds.
export { AutoApprovalMode, FullAutoErrorMode } from "./auto-approval-mode.ts";
