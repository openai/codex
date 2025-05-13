// Note that "../package.json" is marked external in build.mjs. This ensures
// that the contents of package.json will always be read at runtime, which is
// preferable so we do not have to make a temporary change to package.json in
// the source tree to update the version number in the code.
import pkg from "../package.json" with { type: "json" };

// Read the version directly from package.json.
export const CLI_VERSION: string = (pkg as { version: string }).version;
