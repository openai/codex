// Project‑local declaration stubs for external libraries that do not ship
// with TypeScript type definitions. These are intentionally minimal – they
// cover only the APIs that the Codex codebase relies on. If full type
// packages (e.g. `@types/shell‑quote`) are introduced later these stubs will
// be overridden automatically by the higher‑priority package typings.

declare module "shell-quote" {
  /**
   * Very small subset of the return tokens produced by `shell‑quote` that are
   * relevant for our inspection of shell operators. A token can either be a
   * simple string (command/argument) or an operator object such as
   * `{ op: "&&" }`.
   */
  export type Token = string | { op: string };

  // Historically the original `shell-quote` library exports several internal
  // type definitions. We recreate the few that Codex‑Lib imports so that the
  // TypeScript compiler can resolve them.

  /*
   * The real `shell‑quote` types define `ControlOperator` as the literal set
   * of operator strings that can appear in the parsed output. Re‑creating the
   * exhaustive union is unnecessary for our purposes – modelling it as a
   * plain string is sufficient for type‑checking the Codex codebase while
   * still preserving basic safety (the operator string gets validated at
   * runtime anyway).
   */
  export type ControlOperator = "&&" | "||" | "|" | ";" | string;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export type ParseEntry = string | { op: ControlOperator } | any;

  /**
   * Parse a shell command string into tokens. The implementation provided by
   * the `shell‑quote` package supports additional token kinds (glob, comment,
   * redirection …) which we deliberately omit here because Codex never
   * inspects them.
   */
  export function parse(
    cmd: string,
    env?: Record<string, string | undefined>,
  ): Array<Token>;

  /**
   * Quote an array of arguments such that it can be copied & pasted into a
   * POSIX‑compatible shell.
   */
  export function quote(args: ReadonlyArray<string>): string;
}

declare module "diff" {
  /**
   * Minimal stub for the `diff` library which we use only for generating a
   * unified patch between two in‑memory strings.
   */
  export function createTwoFilesPatch(
    oldFileName: string,
    newFileName: string,
    oldStr: string,
    newStr: string,
    oldHeader?: string,
    newHeader?: string,
    options?: { context?: number },
  ): string;
}
