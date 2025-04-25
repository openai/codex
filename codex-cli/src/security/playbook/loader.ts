import fs from "fs";
import path from "path";
import yaml from "js-yaml";
import { PlaybookSchema, Playbook } from "./types";

/**
 * Load and validate a playbook from YAML or JSON.
 * @param filePath Path to the YAML or JSON playbook file
 * @returns Parsed and validated Playbook object
 */
export function loadPlaybook(filePath: string): Playbook {
  const absPath = path.isAbsolute(filePath)
    ? filePath
    : path.resolve(process.cwd(), filePath);
  const content = fs.readFileSync(absPath, "utf-8");
  let raw: any;
  try {
    raw = JSON.parse(content);
  } catch {
    raw = yaml.load(content);
  }
  // Support legacy puppeteer action syntax: { type: { ... } }
  if (raw && typeof raw === 'object' && Array.isArray((raw as any).steps)) {
    const validTypes = ['type', 'click', 'waitForNavigation', 'extractCookie'];
    for (const step of (raw as any).steps) {
      if (step.puppeteer && Array.isArray(step.puppeteer.actions)) {
        step.puppeteer.actions = step.puppeteer.actions.map((act: any) => {
          if (
            act && typeof act === 'object' &&
            Object.keys(act).length === 1 &&
            validTypes.includes(Object.keys(act)[0]) &&
            typeof act[Object.keys(act)[0]] === 'object'
          ) {
            const key = Object.keys(act)[0];
            return { type: key, ...act[key] };
          }
          return act;
        });
      }
    }
  }
  const result = PlaybookSchema.safeParse(raw);
  if (!result.success) {
    throw new Error(
      `Invalid playbook schema for '${filePath}': ${result.error.message}`
    );
  }
  return result.data;
}