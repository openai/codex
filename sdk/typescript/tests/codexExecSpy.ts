import * as child_process from "node:child_process";

jest.mock("node:child_process", () => {
  const actual = jest.requireActual<typeof import("node:child_process")>("node:child_process");
  return { ...actual, spawn: jest.fn(actual.spawn) };
});

const actualChildProcess =
  jest.requireActual<typeof import("node:child_process")>("node:child_process");
const spawnMock = child_process.spawn as jest.MockedFunction<typeof actualChildProcess.spawn>;

export function codexExecSpy(): {
  args: string[][];
  envs: (Record<string, string> | undefined)[];
  restore: () => void;
} {
  const previousImplementation = spawnMock.getMockImplementation() ?? actualChildProcess.spawn;
  const args: string[][] = [];
  const envs: (Record<string, string> | undefined)[] = [];

  spawnMock.mockImplementation(((...spawnArgs: Parameters<typeof child_process.spawn>) => {
    const commandArgs = spawnArgs[1];
    args.push(Array.isArray(commandArgs) ? [...commandArgs] : []);
    const options = spawnArgs[2] as child_process.SpawnOptions | undefined;
    envs.push(options?.env as Record<string, string> | undefined);
    return previousImplementation(...spawnArgs);
  }) as typeof actualChildProcess.spawn);

  return {
    args,
    envs,
    restore: () => {
      spawnMock.mockClear();
      spawnMock.mockImplementation(previousImplementation);
    },
  };
}
