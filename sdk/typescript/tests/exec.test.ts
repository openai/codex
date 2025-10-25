import { beforeEach, describe, expect, it } from "@jest/globals";
import type { ChildProcess } from "node:child_process";
import * as child_process from "node:child_process";
import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";

import { CodexExec } from "../src/exec";

jest.mock("node:child_process", () => {
  const actual = jest.requireActual<typeof import("node:child_process")>("node:child_process");
  return { ...actual, spawn: jest.fn() };
});

const actualChildProcess =
  jest.requireActual<typeof import("node:child_process")>("node:child_process");
const spawnMock = child_process.spawn as jest.MockedFunction<typeof actualChildProcess.spawn>;

class FakeChildProcess extends EventEmitter {
  public stdin: PassThrough | null = new PassThrough();
  public stdout: PassThrough | null = new PassThrough();
  public stderr: PassThrough | null = new PassThrough();
  public killed = false;
  public kill = jest.fn((_signal?: NodeJS.Signals | number) => {
    this.killed = true;
    return true;
  });
}

describe("CodexExec", () => {
  beforeEach(() => {
    spawnMock.mockReset();
  });

  it("rejects pending iterations when aborted", async () => {
    const fakeChild = new FakeChildProcess();
    spawnMock.mockReturnValue(fakeChild as unknown as ChildProcess);

    const exec = new CodexExec("/fake/path");
    const controller = new AbortController();
    const generator = exec.run({ input: "hello", signal: controller.signal });

    const first = generator.next();
    fakeChild.stdout?.write("{\"type\":\"turn.started\"}\n");
    await expect(first).resolves.toEqual({
      value: "{\"type\":\"turn.started\"}",
      done: false,
    });

    const pending = generator.next();
    const abortError = new Error("stop");
    controller.abort(abortError);
    fakeChild.stdout?.end();
    fakeChild.emit("exit", null, "SIGTERM");

    await expect(pending).rejects.toBe(abortError);
    expect(fakeChild.kill).toHaveBeenCalled();
  });
});
