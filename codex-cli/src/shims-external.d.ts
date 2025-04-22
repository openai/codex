// Ambient module declarations for optional/runtime‑only dependencies so that
// `tsc --noEmit` succeeds without installing their full type definitions.

declare module "package-manager-detector" {
  export type AgentName = "npm" | "pnpm" | "yarn" | "bun" | "deno";

  /** Detects the package manager based on environment variables. */
  export function getUserAgent(): AgentName | null | undefined;
}

declare module "fast-npm-meta" {
  export interface LatestVersionMeta {
    version: string;
  }

  export function getLatestVersion(
    pkgName: string,
    opts?: Record<string, unknown>,
  ): Promise<LatestVersionMeta | { error: unknown }>;
}

declare module "semver" {
  export function gt(v1: string, v2: string): boolean;
}
