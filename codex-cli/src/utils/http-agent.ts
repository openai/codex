import { HttpsProxyAgent } from "https-proxy-agent";

/**
 * Returns an appropriate HTTP(S) agent for OpenAI/Azure clients,
 * handling insecure mode and proxy settings.
 *
 * @param config - Configuration object, may contain 'insecure' boolean property.
 * @param proxyUrl - Optional proxy URL (e.g., from process.env.HTTPS_PROXY).
 * @returns An instance of HttpsProxyAgent or undefined.
 */
export function getHttpAgent(config: unknown, proxyUrl?: string) {
  const insecure =
    typeof (config as any).insecure === "boolean"
      ? (config as any).insecure
      : process.env["OPENAI_INSECURE"] === "true";

  if (insecure) {
    // If proxyUrl is set, use it and set rejectUnauthorized: false
    if (proxyUrl) {
      // Build AgentOptions with proxy info and rejectUnauthorized: false
      const proxyOpts = parseProxyUrl(proxyUrl);
      const agentOptions: Record<string, any> = {
        rejectUnauthorized: false,
      };
      if (proxyOpts["host"]) agentOptions["host"] = proxyOpts["host"];
      if (proxyOpts["port"] !== undefined)
        agentOptions["port"] = proxyOpts["port"];
      if (proxyOpts["auth"]) agentOptions["auth"] = proxyOpts["auth"];
      return new HttpsProxyAgent(agentOptions);
    }
    // No proxy, just skip cert verification
    return new HttpsProxyAgent({ rejectUnauthorized: false });
  }
  // Secure mode: use proxy if present, else undefined
  return proxyUrl ? new HttpsProxyAgent(proxyUrl as string) : undefined;
}

/**
 * Parses a proxy URL string into an options object for HttpsProxyAgent.
 * Only includes host, port, protocol, auth if present.
 */
function parseProxyUrl(proxyUrl: string): Record<string, any> {
  try {
    const url = new URL(proxyUrl);
    const options: Record<string, any> = {};
    if (url.hostname) options["host"] = url.hostname;
    if (url.port) options["port"] = Number(url.port);
    if (url.username || url.password) {
      options["auth"] = `${url.username}:${url.password}`;
    }
    return options;
  } catch {
    // fallback: let HttpsProxyAgent handle invalid URL
    return {};
  }
}
