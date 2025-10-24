function formatConfigValue(value: unknown): string {
  if (value === undefined) {
    throw new Error("Cannot set configuration override to undefined");
  }
  return JSON.stringify(value);
}

export class ConfigOverrideStore {
  private readonly overrides = new Map<string, string>();

  set(key: string, value: unknown): void {
    const formatted = formatConfigValue(value);
    this.overrides.set(key, formatted);
  }

  delete(key: string): void {
    this.overrides.delete(key);
  }

  clear(): void {
    this.overrides.clear();
  }

  get(key: string): string | undefined {
    return this.overrides.get(key);
  }

  entries(): IterableIterator<[string, string]> {
    return this.overrides.entries();
  }

  toCliArgs(): string[] {
    const args: string[] = [];
    for (const [key, value] of this.overrides) {
      args.push("--config", `${key}=${value}`);
    }
    return args;
  }
}
