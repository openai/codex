import { describe, it, expect } from "vitest";
import { parsePromptForHiddenFiles } from "../src/utils/singlepass/prompt_parser";

describe("parsePromptForHiddenFiles", () => {
  it("parses a prompt with a single #hidden directive", () => {
    const prompt = "Help me\n#hidden: secret.txt, .env";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Help me");
    expect(result.hiddenPatterns).toEqual(["secret.txt", ".env"]);
  });

  it("parses a prompt with multiple #hidden directives", () => {
    const prompt =
      "Start\n#hidden: foo.js\nContinue\n#hidden: bar.txt, baz.json";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Start\nContinue");
    expect(result.hiddenPatterns).toEqual(["foo.js", "bar.txt", "baz.json"]);
  });

  it("handles prompts with no #hidden directive", () => {
    const prompt = "Just code please!";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Just code please!");
    expect(result.hiddenPatterns).toEqual([]);
  });

  it("trims whitespace and ignores empty patterns", () => {
    const prompt = "Test\n#hidden:  foo.js  ,   , bar.txt , ";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Test");
    expect(result.hiddenPatterns).toEqual(["foo.js", "bar.txt"]);
  });

  it("removes #hidden directive even if at the end", () => {
    const prompt = "Prompt\n#hidden: foo.js";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Prompt");
    expect(result.hiddenPatterns).toEqual(["foo.js"]);
  });

  it("handles #hidden directive with no patterns", () => {
    const prompt = "Prompt\n#hidden:";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Prompt");
    expect(result.hiddenPatterns).toEqual([]);
  });

  it("handles #hidden directive with only spaces", () => {
    const prompt = "Prompt\n#hidden:   ";
    const result = parsePromptForHiddenFiles(prompt);
    expect(result.cleanedPrompt).toBe("Prompt");
    expect(result.hiddenPatterns).toEqual([]);
  });
});
