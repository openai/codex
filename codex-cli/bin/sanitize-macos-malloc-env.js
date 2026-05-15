const MACOS_MALLOC_DIAGNOSTIC_ENV_PREFIXES = [
  "MallocStackLogging",
  "MallocLogFile",
];

export function sanitizeMacosMallocDiagnosticEnv(env, platform = process.platform) {
  if (platform !== "darwin") {
    return env;
  }

  for (const key of Object.keys(env)) {
    if (
      MACOS_MALLOC_DIAGNOSTIC_ENV_PREFIXES.some((prefix) =>
        key.startsWith(prefix),
      )
    ) {
      delete env[key];
    }
  }

  return env;
}
