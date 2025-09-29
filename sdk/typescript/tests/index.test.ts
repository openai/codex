import { describe, expect, it } from "vitest";

import { Codex } from "../src/index.js";

describe("Codex", () => {
  it("exposes the placeholder API", () => {
    const client = new Codex();
  });
});
