/*
 * Centralised access to environment variables used by the Codex GitHub
 * Action.
 *
 * To enable proper unit-testing we avoid reading from `process.env` at module
 * initialisation time.  Instead a `EnvContext` object is created (usually from
 * the real `process.env`) and passed around explicitly or – where that is not
 * yet practical – imported as the shared `defaultContext` singleton. Tests can
 * create their own context backed by a stubbed map of variables without having
 * to mutate global state.
 */

import { fail } from "./fail";
import * as github from "@actions/github";

export interface EnvContext {
  /**
   * Return the value for a given environment variable or terminate the action
   * via `fail` if it is missing / empty.
   */
  get(name: string): string;

  /**
   * Attempt to read an environment variable. Returns the value when present;
   * otherwise returns undefined (does not call `fail`).
   */
  tryGet(name: string): string | undefined;

  /**
   * Attempt to read an environment variable. Returns non-empty string value or
   * null if unset or empty string.
   */
  tryGetNonEmpty(name: string): string | null;

  /**
   * Return a memoised Octokit instance authenticated via the token resolved
   * from the provided argument (when defined) or the environment variables
   * `GITHUB_TOKEN`/`GH_TOKEN`.
   *
   * Subsequent calls return the same cached instance to avoid spawning
   * multiple REST clients within a single action run.
   */
  getOctokit(token?: string): ReturnType<typeof github.getOctokit>;
}

/** Internal helper – *not* exported. */
function _getRequiredEnv(
  name: string,
  env: Record<string, string | undefined>,
): string | undefined {
  const value = env[name];

  // Avoid leaking secrets into logs while still logging non-secret variables.
  if (name.endsWith("KEY") || name.endsWith("TOKEN")) {
    if (value) {
      console.log(`value for ${name} was found`);
    }
  } else {
    console.log(`${name}=${value}`);
  }

  return value;
}

/** Create a context backed by the supplied environment map (defaults to `process.env`). */
export function createEnvContext(
  env: Record<string, string | undefined> = process.env,
): EnvContext {
  // Lazily instantiated Octokit client – shared across this context.
  let cachedOctokit: ReturnType<typeof github.getOctokit> | null = null;

  return {
    get(name: string): string {
      const value = _getRequiredEnv(name, env);
      if (value == null) {
        fail(`Missing required environment variable: ${name}`);
      }
      return value;
    },

    tryGet(name: string): string | undefined {
      return _getRequiredEnv(name, env);
    },

    tryGetNonEmpty(name: string): string | null {
      const value = _getRequiredEnv(name, env);
      return value == null || value === "" ? null : value;
    },

    getOctokit(token?: string) {
      if (cachedOctokit) {
        return cachedOctokit;
      }

      // Determine the token to authenticate with.
      const githubToken = token ?? env["GITHUB_TOKEN"] ?? env["GH_TOKEN"];

      if (!githubToken) {
        fail(
          "Unable to locate a GitHub token. `github_token` should have been set on the action.",
        );
      }

      cachedOctokit = github.getOctokit(githubToken!);
      return cachedOctokit;
    },
  };
}

/**
 * Shared context built from the actual `process.env`.  Production code that is
 * not yet refactored to receive a context explicitly may import and use this
 * singleton.  Tests should avoid the singleton and instead pass their own
 * context to the functions they exercise.
 */
export const defaultContext: EnvContext = createEnvContext();
