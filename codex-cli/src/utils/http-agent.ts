import { HttpsProxyAgent } from "https-proxy-agent";

type ConfigWithInsecure = { insecure?: boolean };

/**
 * Returns an appropriate HTTP(S) agent for OpenAI/Azure clients,
 * handling insecure mode and proxy settings.
 *
 * @param config - Configuration object, may contain 'insecure' boolean property.
 * @param proxyUrl - Optional proxy URL (e.g., from process.env.HTTPS_PROXY).
 * @returns An instance of HttpsProxyAgent or undefined.
 */
export function getHttpAgent(
  config: unknown,
  proxyUrl?: string,
): HttpsProxyAgent<string> | undefined {
  const insecure =
    typeof (config as ConfigWithInsecure).insecure === "boolean"
      ? (config as ConfigWithInsecure).insecure
      : process.env["OPENAI_INSECURE"] === "true";

  if (insecure) {
    // In insecure mode, skip certificate verification globally
    process.env["NODE_TLS_REJECT_UNAUTHORIZED"] = "0";
    // Use proxy if present
    return proxyUrl ? new HttpsProxyAgent(proxyUrl) : undefined;
  }
  // Secure mode: use proxy if present, else undefined
  return proxyUrl ? new HttpsProxyAgent(proxyUrl) : undefined;
}
