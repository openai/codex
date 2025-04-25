import fs from 'fs';
import path from 'path';
import readline from 'readline';
import yaml from 'js-yaml';
import type { AppConfig } from '../../utils/config';
import { loadPlaybook } from './loader';
import { runPlaybookSession } from './index';
import OpenAI from 'openai';

/**
 * Interactive LLM-driven playbook generation flow.
 */
export async function runPlaybookGenerator(
  prompt: string | undefined,
  config: AppConfig,
  options: { target?: string; session?: string }
): Promise<void> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  const ask = (q: string) => new Promise<string>(res => rl.question(q, ans => res(ans.trim())));

  console.log('\n🛠️  Generating a new playbook');
  // Ask for target if missing
  let target = options.target;
  while (!target) {
    target = await ask('Target URL or IP (e.g. https://api.example.com): ');
    if (!target) console.log('Target is required.');
  }
  // Authorization
  let auth = '';
  while (auth.toLowerCase() !== 'yes') {
    auth = await ask('Authorization attestation (YES to continue): ');
    if (auth.toLowerCase() !== 'yes') console.error('Must type YES to confirm authorization.');
  }
  // Objective
  const objective = await ask('Objective (e.g. test for admin access bypass): ');
  if (!objective) {
    console.error('Objective is required.');
    process.exit(1);
  }

  // Call OpenAI to draft playbook YAML
  console.log('\n🔮 Asking LLM to draft playbook steps...');
  const openai = new OpenAI({ apiKey: config.apiKey });
  const system = `You are an AI assistant that generates security playbooks ` +
    `in YAML format following the Cyber Kill Chain. Respond ONLY with valid YAML.`;
  const userPrompt = `Generate a YAML playbook with id, name, mode: predator, retry_on_failure: true, ` +
    `and 3-5 steps for objective '${objective}' against target '${target}'. ` +
    `Each step needs phase, description, action(method, path), headers/payload if needed, ` +
    `optional extract or validate blocks.`;
  const res = await openai.chat.completions.create({
    model: config.model,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: userPrompt }
    ]
  });
  const yamlText = res.choices?.[0]?.message?.content ?? '';

  // Validate generated YAML against schema
  let pb;
  try {
    const raw = yaml.load(yamlText);
    // Use loader logic on raw object (bypass file reading)
    // We can parse directly via loader by writing temp file, but better to validate schema here
    const { PlaybookSchema } = await import('./types');
    const parsed = PlaybookSchema.safeParse(raw);
    if (!parsed.success) throw new Error(parsed.error.message);
    pb = parsed.data;
  } catch (err: any) {
    console.error('LLM output is not a valid playbook:', err.message);
    console.error('Generated output:\n', yamlText);
    process.exit(1);
  }

  // Save file
  const playbooksDir = path.join(process.env.HOME || process.cwd(), '.adversys', 'playbooks');
  if (!fs.existsSync(playbooksDir)) fs.mkdirSync(playbooksDir, { recursive: true });
  const outPath = path.join(playbooksDir, `${pb.id}.yaml`);
  fs.writeFileSync(outPath, yamlText, 'utf-8');
  console.log(`\n✅ Playbook generated and saved to ${outPath}`);

  // Ask to run now
  const runAnswer = await ask('Run this playbook now? (y/N): ');
  rl.close();
  if (/^y(es)?$/i.test(runAnswer.trim())) {
    await runPlaybookSession(outPath, config, { target, session: options.session });
  }
}