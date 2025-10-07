import { loadGuardrails } from "./extensions/guardrails.ts";

export const GUARDRAILS_ENV_VAR = "CODEX_GUARDRAILS";

const GUARDRAIL_FLAG = "--guardrails";
const GUARDRAIL_FLAG_ALIAS = "-g";
const GUARDRAIL_FLAG_DISABLE = "--no-guardrails";

function parseBoolean(value) {
  if (value == null) {
    return null;
  }

  const normalized = String(value).trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }

  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }

  return null;
}

function parseGuardrailFlag(argv) {
  for (const arg of argv) {
    if (arg === GUARDRAIL_FLAG || arg === GUARDRAIL_FLAG_ALIAS) {
      return true;
    }

    if (arg === GUARDRAIL_FLAG_DISABLE) {
      return false;
    }

    if (arg.startsWith("--guardrails=")) {
      const [, rawValue] = arg.split("=", 2);
      const parsed = parseBoolean(rawValue);
      if (parsed !== null) {
        return parsed;
      }
      return rawValue !== "";
    }
  }

  return null;
}

export function shouldUseGuardrails({ argv = process.argv.slice(2), env = process.env } = {}) {
  const cliPreference = parseGuardrailFlag(argv);
  if (cliPreference !== null) {
    return cliPreference;
  }

  const envPreference = parseBoolean(env[GUARDRAILS_ENV_VAR]);
  return envPreference === null ? false : envPreference;
}

export async function buildPromptWithGuardrails(userPrompt, options = {}) {
  const { argv = process.argv.slice(2), env = process.env, cwd = process.cwd(), guardrailsEnabled } = options;
  const enabled =
    typeof guardrailsEnabled === "boolean" ? guardrailsEnabled : shouldUseGuardrails({ argv, env });

  if (!enabled) {
    return userPrompt;
  }

  const guardrails = await loadGuardrails({ cwd, prompt: userPrompt });
  if (!guardrails) {
    return userPrompt;
  }

  return `${guardrails}\n\n${userPrompt}`;
}
