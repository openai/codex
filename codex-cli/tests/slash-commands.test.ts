import { test, expect } from "vitest";
import { SLASH_COMMANDS } from "../src/utils/slash-commands.js";

test("SLASH_COMMANDS includes expected commands", () => {
  const commands = SLASH_COMMANDS.map((c) => c.command);
  expect(commands).toContain("/clear");
  expect(commands).toContain("/compact");
  expect(commands).toContain("/config");
  expect(commands).toContain("/cost");
  expect(commands).toContain("/history");
  expect(commands).toContain("/help");
  expect(commands).toContain("/model");
  expect(commands).toContain("/approval");
  expect(commands).toContain("/clearhistory");
});

test("filters slash commands by prefix", () => {
  const prefix = "/c";
  const filtered = SLASH_COMMANDS.filter((c) => c.command.startsWith(prefix));
  const names = filtered.map((c) => c.command);
  // Should include /clear, /compact, /config, /cost, /clearhistory
  expect(names).toEqual(
    expect.arrayContaining([
      "/clear",
      "/compact",
      "/config",
      "/cost",
      "/clearhistory",
    ])
  );
});