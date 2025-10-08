export function normalizePathForDisplay(value: string): string {
  return value.replace(/\\/g, "/");
}

export function normalizePathArrayForDisplay(values: ReadonlyArray<string>): Array<string> {
  return values.map((value) => normalizePathForDisplay(value));
}
