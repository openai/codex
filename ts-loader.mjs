import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export async function resolve(specifier, context, defaultResolve) {
  if (specifier.endsWith(".ts")) {
    const url = new URL(specifier, context.parentURL || pathToFileURL(process.cwd() + "/"));
    return { url: url.href, format: "module", shortCircuit: true };
  }

  return defaultResolve(specifier, context, defaultResolve);
}

export async function load(url, context, defaultLoad) {
  if (url.endsWith(".ts")) {
    const source = await readFile(new URL(url), "utf8");
    return { format: "module", source, shortCircuit: true };
  }

  return defaultLoad(url, context, defaultLoad);
}
