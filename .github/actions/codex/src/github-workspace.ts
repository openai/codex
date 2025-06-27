import * as pathMod from "path";
import { EnvContext } from "./env-context";

export function resolveWorkspacePath(path: string, ctx: EnvContext): string {
  if (pathMod.isAbsolute(path)) {
    return path;
  } else {
    const workspace = ctx.get("GITHUB_WORKSPACE");
    return pathMod.join(workspace, path);
  }
}
