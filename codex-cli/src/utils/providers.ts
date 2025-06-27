import { providerConfigs } from "../providers/configs.js";

/**
 * Legacy provider format for backward compatibility
 * New code should use the provider registry directly
 */
export const providers: Record<
  string,
  { name: string; baseURL: string; envKey: string }
> = Object.entries(providerConfigs).reduce(
  (acc, [id, config]) => {
    acc[id] = {
      name: config.name,
      baseURL: config.baseURL,
      envKey: config.envKey,
    };
    return acc;
  },
  {} as Record<string, { name: string; baseURL: string; envKey: string }>,
);
