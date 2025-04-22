import { PassThrough } from "stream";
import { once } from "events";
import { describe, it, expect } from "vitest";
import { createTruncatingCollector } from "../src/utils/agent/sandbox/create-truncating-collector.js";

describe("createTruncatingCollector", () => {
  it("collects data under limits without truncation", async () => {
    const stream = new PassThrough();
    const collector = createTruncatingCollector(stream, 100, 10);
    const data = "line1\nline2\n";
    stream.end(Buffer.from(data));
    await once(stream, "end");
    expect(collector.getString()).toBe(data);
    expect(collector.hit).toBe(false);
  });

  it("truncates data over byte limit", async () => {
    const stream = new PassThrough();
    const collector = createTruncatingCollector(stream, 5, 100);
    stream.end(Buffer.from("hello world"));
    await once(stream, "end");
    expect(collector.getString()).toBe("hello");
    expect(collector.hit).toBe(true);
  });

  it("truncates data over line limit", async () => {
    const stream = new PassThrough();
    const collector = createTruncatingCollector(stream, 1000, 2);
    const data = "a\nb\nc\nd\n";
    stream.end(Buffer.from(data));
    await once(stream, "end");
    expect(collector.getString()).toBe("a\nb\n");
    expect(collector.hit).toBe(true);
  });

  it("stops collecting after limit is hit across multiple writes", async () => {
    const stream = new PassThrough();
    const collector = createTruncatingCollector(stream, 10, 2);
    stream.write(Buffer.from("1\n"));
    stream.write(Buffer.from("2\n3\n4\n"));
    stream.end();
    await once(stream, "end");
    expect(collector.getString()).toBe("1\n2\n");
    expect(collector.hit).toBe(true);
  });

  it("handles zero limits", async () => {
    const stream = new PassThrough();
    const collector = createTruncatingCollector(stream, 0, 0);
    stream.end(Buffer.from("anything\n"));
    await once(stream, "end");
    expect(collector.getString()).toBe("");
    expect(collector.hit).toBe(true);
  });
});
