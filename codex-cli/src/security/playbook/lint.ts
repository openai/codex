import fs from 'fs';
import path from 'path';
import yaml from 'js-yaml';
import { loadPlaybook } from './loader';
import { Playbook } from './types';

/**
 * Validate a playbook file and optionally explain its contents.
 * @param filePath path to YAML/JSON playbook
 * @param explain whether to print a natural-language summary
 */
export async function lintPlaybook(filePath: string, explain: boolean): Promise<void> {
  const absPath = path.isAbsolute(filePath)
    ? filePath
    : path.resolve(process.cwd(), filePath);
  if (!fs.existsSync(absPath)) {
    throw new Error(`Playbook file not found: ${filePath}`);
  }
  let pb: Playbook;
  try {
    pb = loadPlaybook(absPath);
  } catch (err: any) {
    throw new Error(err.message);
  }
  console.log(`✅ Playbook '${pb.id}' schema is valid (${pb.steps.length} steps)`);
  if (explain) {
    console.log('\nPlaybook overview:');
    console.log(`- ID: ${pb.id}`);
    if (pb.name) console.log(`- Name: ${pb.name}`);
    console.log(`- Mode: ${pb.mode}`);
    console.log(`- Retry on failure: ${pb.retry_on_failure ?? false}`);
    console.log('- Steps:');
    pb.steps.forEach((step, idx) => {
      console.log(`  ${idx+1}. [${step.phase}] ${step.action.method} ${step.action.path}`);
      if (step.description) console.log(`       Description: ${step.description}`);
      if (step.headers) console.log(`       Headers: ${JSON.stringify(step.headers)}`);
      if (step.payload) console.log(`       Payload: ${JSON.stringify(step.payload)}`);
      if (step.extract) console.log(`       Extract: '${step.extract.path}' -> ${step.extract.save_as}`);
      if (step.validate) console.log(`       Validate: ${JSON.stringify(step.validate)}`);
      console.log('');
    });
  }
}