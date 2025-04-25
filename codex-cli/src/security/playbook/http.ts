import fetch from "node-fetch";
import type { Step } from "./types";
import { VarManager } from "./vars";

/**
 * HTTP client for executing playbook steps against a base target.
 */
export class HttpClient {
  private baseUrl: string;
  private vars: VarManager;

  constructor(target: string, vars: VarManager) {
    // Trim any trailing slash from target
    this.baseUrl = target.replace(/\/+$/, "");
    this.vars = vars;
  }

  /**
   * Execute an HTTP request for the given playbook step.
   */
  async request(step: Step): Promise<{ status: number; headers: Record<string, string>; body: any }> {
    const { method, path } = step.action;
    const relPath = this.vars.substitute(path);
    const url = this.baseUrl + relPath;
    // Prepare headers
    const hdrs: Record<string, string> = {};
    if (step.headers) {
      for (const [k, v] of Object.entries(step.headers)) {
        hdrs[k] = this.vars.substitute(v);
      }
    }
    // Prepare body
    let body: any = undefined;
    if (step.payload !== undefined) {
      const payload = this.vars.substitute(step.payload);
      body = JSON.stringify(payload);
      hdrs["Content-Type"] = hdrs["Content-Type"] || "application/json";
    }
    // Execute fetch
    const res = await fetch(url, {
      method,
      headers: hdrs,
      body,
    });
    // Collect response
    const respHeaders: Record<string, string> = {};
    res.headers.forEach((v, k) => { respHeaders[k] = v; });
    // Parse body
    const contentType = res.headers.get("content-type") || "";
    let data: any;
    if (contentType.includes("application/json")) {
      data = await res.json();
    } else {
      data = await res.text();
    }
    return { status: res.status, headers: respHeaders, body: data };
  }
}