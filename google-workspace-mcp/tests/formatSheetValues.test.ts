import { formatSheetValues } from "../src/index";

describe("formatSheetValues", () => {
  it("returns empty collections for non-array input", () => {
    const result = formatSheetValues(null);
    expect(result.rows).toEqual([]);
    expect(result.lines).toEqual([]);
  });

  it("coerces cell values to strings and joins rows with tabs", () => {
    const result = formatSheetValues([
      ["A", "B", 3],
      [null, undefined, "text"],
    ]);

    expect(result.rows).toEqual([
      ["A", "B", "3"],
      ["", "", "text"],
    ]);
    expect(result.lines).toEqual(["A\tB\t3", "\t\ttext"]);
  });
});
