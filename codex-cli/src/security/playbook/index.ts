import path from "path";
import chalk from "chalk";
import type { AppConfig } from "../utils/config";
import { loadPlaybook } from "./loader";
import { VarManager } from "./vars";
import { HttpClient } from "./http";
import { PlaybookRunner, RunReport } from "./runner";

/**
 * Options for running a playbook session.
 */
export interface PlaybookSessionOptions {
  target?: string;
  session?: string;
  dryRun?: boolean;
}

/**
 * Execute or dry-run a playbook file in Predator mode.
 */
export async function runPlaybookSession(
  playbookPath: string,
  config: AppConfig,
  options: PlaybookSessionOptions
): Promise<void> {
  // Load and validate the playbook
  const absPath = path.isAbsolute(playbookPath)
    ? playbookPath
    : path.resolve(process.cwd(), playbookPath);
  const pb = loadPlaybook(absPath);
  console.log(`Loaded playbook '${pb.id}' (${pb.steps.length} steps)`);
  // Ensure target is set
  if (!options.target) {
    throw new Error("--target is required when running a playbook.");
  }
  const vars = new VarManager();
  const http = new HttpClient(options.target, vars);
  // Dry-run: list steps without execution
  if (options.dryRun) {
    console.log("\nDry-run mode: Plan of execution:");
    pb.steps.forEach((step, idx) => {
      const num = idx + 1;
      if (step.puppeteer) {
        const urlText = step.puppeteer.url ? ` url=${step.puppeteer.url}` : '';
        console.log(`${num}. [${step.phase}] PUPPETEER browser automation (${step.puppeteer.actions.length} actions)${urlText}`);
        step.puppeteer.actions.forEach((act, j) => {
          let desc = `    ${j + 1}. ${act.type}`;
          if ('selector' in act) desc += ` selector=${act.selector}`;
          if ('text' in act) desc += ` text=${act.text}`;
          if (act.type === 'extractCookie') desc += ` name=${act.name} => ${act.save_as}`;
          console.log(desc);
        });
      } else {
        const p = vars.substitute(step.action.path);
        console.log(`${num}. [${step.phase}] ${step.action.method} ${p}`);
        if (step.description) console.log(`    desc: ${step.description}`);
        if (step.headers) console.log(`    headers: ${JSON.stringify(vars.substitute(step.headers))}`);
        if (step.payload) console.log(`    payload: ${JSON.stringify(vars.substitute(step.payload))}`);
      }
    });
    return;
  }
  // Execute playbook with live logging
  const runStart = Date.now();
  const runner = new PlaybookRunner(
    pb,
    http,
    vars,
    options.session,
    (step) => {
      const { id, phase, method, path: p, status, success, retries, durationMs, extracted } = step;
      const symbol = success ? '✅' : retries > 1 ? '⚠️' : '❌';
      const colorFn = success ? chalk.green : chalk.red;
      console.log(
        colorFn(
          `${symbol} [${id}] ${phase} ${method} ${p} -> ${status} ` +
            `(${(durationMs/1000).toFixed(2)}s, ${retries} attempt${retries>1?'s':''})`
        )
      );
      for (const [k, v] of Object.entries(extracted)) {
        console.log(chalk.blue(`    ✔ ${k} = ${v}`));
      }
    }
  );
  const report: RunReport = await runner.run();
  // Write JSON report
  const { homedir } = await import("os");
  const { join } = await import("path");
  const fs = await import("fs");
  const reportsDir = join(homedir(), ".adversys", "reports");
  if (!fs.existsSync(reportsDir)) fs.mkdirSync(reportsDir, { recursive: true });
  const jsonPath = join(reportsDir, `${pb.id}.json`);
  fs.writeFileSync(jsonPath, JSON.stringify(report, null, 2));
  console.log(`Report written: ${jsonPath}`);
  // Write markdown summary
  const mdLines: string[] = [];
  mdLines.push(`# Playbook Report: ${pb.id}`);
  if (pb.name) mdLines.push(`**Name**: ${pb.name}`);
  mdLines.push(`**Timestamp**: ${report.timestamp}`);
  mdLines.push("\n| # | Step ID | Phase | Method | Path | Status | Success | Retries | Extracted |");
  mdLines.push("|---|---------|-------|--------|------|--------|---------|-----------|");
  report.steps.forEach((sr, i) => {
    const ext = Object.keys(sr.extracted).length ? JSON.stringify(sr.extracted) : "";
    mdLines.push(
      `| ${i + 1} | ${sr.id} | ${sr.phase} | ${sr.method} | ${sr.path} | ${sr.status ?? ""} | ${sr.success} | ${sr.retries} | ${ext} |`
    );
  });
  const mdPath = join(reportsDir, `${pb.id}.md`);
  fs.writeFileSync(mdPath, mdLines.join("\n"));
  console.log(`Markdown summary written: ${mdPath}`);
  // Final summary
  const totalMs = Date.now() - runStart;
  const successCount = report.steps.filter(s => s.success).length;
  const failCount = report.steps.length - successCount;
  console.log(
    chalk.bold(
      `\nPlaybook run complete: ${successCount}/${report.steps.length} steps succeeded, ` +
      `${failCount} failed in ${(totalMs/1000).toFixed(2)}s`
    )
  );
}