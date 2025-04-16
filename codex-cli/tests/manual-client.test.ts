import { parseManualModels } from "../src/utils/manual-client.js";

describe("parseManualModels", () => {
  it("parses multiple lines into model array", () => {
    const input = "gpt-4\no4-mini \n\n text-davinci-003\r\n";
    expect(parseManualModels(input)).toEqual([
      "gpt-4",
      "o4-mini",
      "text-davinci-003",
    ]);
  });

  it("returns empty array for empty input", () => {
    expect(parseManualModels("")).toEqual([]);
  });

  it("trims whitespace and filters empty lines", () => {
    const input = "  model-a  \n   \nmodel-b\n";
    expect(parseManualModels(input)).toEqual(["model-a", "model-b"]);
  });
});