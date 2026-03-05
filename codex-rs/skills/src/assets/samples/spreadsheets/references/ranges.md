# Ranges

Ranges use A1 notation and are the main surface for reading and writing cells.

```js
const sheet = workbook.worksheets.add("Sheet1");

sheet.getRange("A1:C1").values = [["Month", "Bookings", "ARR"]];
sheet.getRange("A2:C4").values = [
  ["Jan", 120000, 1440000],
  ["Feb", 135000, 1620000],
  ["Mar", 142000, 1704000],
];

sheet.getRange("E2").formulas = [["=SUM(C2:C4)"]];
workbook.recalculate();
```

## Notes

- Use `range.values` for literal values.
- Use `range.formulas` for Excel-style formulas.
- Call `workbook.recalculate()` before reading formula-driven `range.values`.
- Apply formatting through `range.format` when the sheet needs borders, fills, fonts, or number formats.

## Addressing

- Single cell: `"B3"`
- Rectangle: `"A1:C10"`
- Entire row or column ranges are best used sparingly unless the user explicitly wants them.
