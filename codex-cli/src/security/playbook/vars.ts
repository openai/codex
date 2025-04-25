/**
 * In-memory variable store and substitution engine for playbooks.
 */
export class VarManager {
  private store = new Map<string, string>();

  /**
   * Save a value under a variable name.
   */
  set(name: string, value: unknown): void {
    this.store.set(name, String(value));
  }

  /**
   * Retrieve a variable's string value, or undefined if not set.
   */
  get(name: string): string | undefined {
    return this.store.get(name);
  }

  /**
   * Recursively substitute {{var}} placeholders in strings, arrays, or objects.
   */
  substitute<T>(input: T): T {
    if (typeof input === "string") {
      return (input.replace(/{{\s*([\w-]+)\s*}}/g, (_m, key) => {
        return this.store.get(key) ?? "";
      }) as unknown) as T;
    }
    if (Array.isArray(input)) {
      return (input.map(item => this.substitute(item)) as unknown) as T;
    }
    if (input && typeof input === "object") {
      const o: any = {};
      for (const [k, v] of Object.entries(input as any)) {
        o[k] = this.substitute(v as any);
      }
      return o as T;
    }
    return input;
  }
}