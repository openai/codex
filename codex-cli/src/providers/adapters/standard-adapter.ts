import type { ProviderConfig, AuthProvider } from "../types.js";

import { BaseAdapter } from "./base-adapter.js";

/**
 * Standard adapter for providers that work with the default OpenAI client
 */
export class StandardAdapter extends BaseAdapter {
  constructor(config: ProviderConfig, authProvider: AuthProvider) {
    super(config, authProvider);
  }
}
