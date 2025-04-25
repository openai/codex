import { Playbook, Step } from "./types";
import { VarManager } from "./vars";
import { HttpClient } from "./http";
import { PuppeteerClient } from './puppeteer';
import { homedir } from "os";
import { join } from "path";
import fs from "fs";

/**
 * Report for an individual playbook step execution.
 */
export interface StepReport {
  id: string;
  phase: string;
  method: string;
  path: string;
  status: number | null;
  success: boolean;
  error?: string;
  retries: number;
  extracted: Record<string, any>;
  /** Duration of the step in milliseconds */
  durationMs: number;
}

/**
 * Aggregate report for an entire playbook run.
 */
export interface RunReport {
  playbookId: string;
  name?: string;
  timestamp: string;
  steps: StepReport[];
}

/**
 * Core engine to run a Playbook.
 */
export class PlaybookRunner {
  /**
   * @param playbook the parsed playbook
   * @param http HTTP client for step requests
   * @param vars variable manager for substitutions
   * @param session optional session name
   * @param onStep optional callback after each step executes
   */
  constructor(
    private playbook: Playbook,
    private http: HttpClient,
    private vars: VarManager,
    private session?: string,
    private onStep?: (step: StepReport) => void
  ) {}

  /**
   * Execute the playbook sequentially and produce a report.
   * Throws on unrecoverable failure.
   */
  async run(): Promise<RunReport> {
    const report: RunReport = {
      playbookId: this.playbook.id,
      name: this.playbook.name,
      timestamp: new Date().toISOString(),
      steps: [],
    };
    // Iterate over steps
    const puppeteerClient = new PuppeteerClient();
    for (let idx = 0; idx < this.playbook.steps.length; idx++) {
      const step = this.playbook.steps[idx];
      // Handle Puppeteer block if present
      if (step.puppeteer) {
        const stepId = step.id ?? `step-${idx + 1}`;
        const stepReport: StepReport = {
          id: stepId,
          phase: step.phase,
          method: 'PUPPETEER',
          path: step.puppeteer.url ?? '',
          status: null,
          success: false,
          retries: 1,
          extracted: {},
          durationMs: 0,
        };
        const startTime = Date.now();
        try {
          await puppeteerClient.run(step.puppeteer, this.vars);
          stepReport.success = true;
        } catch (err: any) {
          stepReport.error = err.message;
        }
        stepReport.durationMs = Date.now() - startTime;
        if (this.onStep) this.onStep(stepReport);
        report.steps.push(stepReport);
        continue;
      }
      const stepId = step.id ?? `step-${idx + 1}`;
      const method = step.action.method;
      const rawPath = step.action.path;
      const substitutedPath = this.vars.substitute(rawPath);
      const stepReport: StepReport = {
        id: stepId,
        phase: step.phase,
        method,
        path: substitutedPath,
        status: null,
        success: false,
        retries: 0,
        extracted: {},
        durationMs: 0,
      };
      const maxAttempts = (step.retry_on_failure ?? this.playbook.retry_on_failure) ? 2 : 1;
      const startTime = Date.now();
      let lastError: string | undefined;
      // Attempt the step
      for (let attempt = 1; attempt <= maxAttempts; attempt++) {
        stepReport.retries = attempt;
        try {
          // Execute HTTP request
          const resp = await this.http.request(step);
          stepReport.status = resp.status;
          // Extraction
          if (step.extract) {
            const val = extractValue(resp.body, step.extract.path);
            this.vars.set(step.extract.save_as, val);
            stepReport.extracted[step.extract.save_as] = val;
          }
          // Validation
          if (step.validate) {
            // Status code
            if (step.validate.status_code != null && resp.status !== step.validate.status_code) {
              throw new Error(`Expected status ${step.validate.status_code}, got ${resp.status}`);
            }
            // Body contains
            if (step.validate.contains != null) {
              const bodyStr = typeof resp.body === 'string' ? resp.body : JSON.stringify(resp.body);
              if (!bodyStr.includes(step.validate.contains)) {
                throw new Error(`Response does not contain '${step.validate.contains}'`);
              }
            }
          }
          // Success
          stepReport.success = true;
          lastError = undefined;
          break;
        } catch (err: any) {
          lastError = err.message;
          if (attempt < maxAttempts) {
            // retry
            continue;
          }
        }
      }
      // After attempts
      if (!stepReport.success) {
        stepReport.error = lastError;
        // Abort if not configured to skip
        if (!(step.retry_on_failure ?? this.playbook.retry_on_failure)) {
          // record duration and notify
          stepReport.durationMs = Date.now() - startTime;
          if (this.onStep) {
            try { this.onStep(stepReport); } catch {}
          }
          report.steps.push(stepReport);
          throw new Error(`Aborting playbook '${this.playbook.id}' at step '${stepId}': ${lastError}`);
        }
      }
      // record duration and callback
      stepReport.durationMs = Date.now() - startTime;
      if (this.onStep) {
        try { this.onStep(stepReport); } catch { /* ignore */ }
      }
      report.steps.push(stepReport);
    }
    return report;
  }
}

/**
 * Extract a nested value from a JSON object using dot notation.
 */
function extractValue(body: any, pathExpr: string): any {
  const parts = pathExpr.split('.').filter(p => p);
  let cursor = body;
  for (const part of parts) {
    if (cursor && typeof cursor === 'object' && part in cursor) {
      cursor = cursor[part];
    } else {
      return undefined;
    }
  }
  return cursor;
}