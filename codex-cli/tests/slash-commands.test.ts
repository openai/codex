import { test, expect } from "vitest";
import { SLASH_COMMANDS, type SlashCommand } from "../src/utils/slash-commands";

test("SLASH_COMMANDS includes expected commands", () => {
  const commands = SLASH_COMMANDS.map((c: SlashCommand) => c.command);
  expect(commands).toContain("/clear");
  expect(commands).toContain("/compact");
  expect(commands).toContain("/history");
  expect(commands).toContain("/sessions");
  expect(commands).toContain("/help");
  expect(commands).toContain("/model");
  expect(commands).toContain("/approval");
  expect(commands).toContain("/clearhistory");
  expect(commands).toContain("/diff");
});

test("filters slash commands by prefix", () => {
  const prefix = "/c";
  const filtered = SLASH_COMMANDS.filter((c: SlashCommand) =>
    c.command.startsWith(prefix),
  );
  const names = filtered.map((c: SlashCommand) => c.command);
  expect(names).toEqual(
    expect.arrayContaining(["/clear", "/clearhistory", "/compact"]),
  );
  expect(names).not.toEqual(
    expect.arrayContaining(["/history", "/help", "/model", "/approval"]),
  );

  const emptyPrefixFiltered = SLASH_COMMANDS.filter((c: SlashCommand) =>
    c.command.startsWith(""),
  );
  const emptyPrefixNames = emptyPrefixFiltered.map(
    (c: SlashCommand) => c.command,
  );
  expect(emptyPrefixNames).toEqual(
    expect.arrayContaining(SLASH_COMMANDS.map((c: SlashCommand) => c.command)),
  );
  expect(emptyPrefixNames).toHaveLength(SLASH_COMMANDS.length);
});
