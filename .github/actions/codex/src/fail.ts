export function fail(message: string): never {
  console.error(message);
  process.exit(1);
}
