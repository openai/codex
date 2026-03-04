---
name: spreadsheets
description: Build, edit, recalculate, import, and export spreadsheet workbooks with the preloaded @oai/artifact-tool JavaScript surface through the artifacts tool.
metadata:
  short-description: Use the artifacts tool to create and edit spreadsheets in JavaScript
---

# Spreadsheets

Use this skill when the user wants to create or modify workbooks with the `artifacts` tool.

## Tool Contract

- Use the `artifacts` tool.
- Send raw JavaScript only. Do not send JSON objects, quoted code, or markdown fences.
- This tool runs plain JavaScript in Node, not TypeScript. Do not use type annotations, `type`, `interface`, or `import type`.
- Do not write `import { ... } from "@oai/artifact-tool"`. The package surface is already preloaded.
- Named exports such as `Workbook`, `SpreadsheetFile`, and `FileBlob` are available directly.
- The full module is also available as `artifactTool`, `artifacts`, and `codexArtifacts`.
- Save outputs under a user-visible path such as `artifacts/revenue-model.xlsx`.

## Quick Start

```js
const workbook = Workbook.create();
const sheet = workbook.worksheets.add("Revenue");

sheet.getRange("A1:C1").values = [["Month", "Bookings", "ARR"]];
sheet.getRange("A2:C4").values = [
  ["Jan", 120000, 1440000],
  ["Feb", 135000, 1620000],
  ["Mar", 142000, 1704000],
];

sheet.getRange("E1").values = [["Quarter ARR"]];
sheet.getRange("E2").formulas = [["=SUM(C2:C4)"]];

workbook.recalculate();

const xlsxBlob = await SpreadsheetFile.exportXlsx(workbook);
await xlsxBlob.save("artifacts/revenue-model.xlsx");
```

## Common Patterns

- Create a workbook with `Workbook.create()`.
- Import an existing workbook with `await SpreadsheetFile.importXlsx(await FileBlob.load("book.xlsx"))`.
- Add sheets with `workbook.worksheets.add(name)`.
- Address cells and ranges with A1 notation via `sheet.getRange("A1:C10")`.
- Set `range.values` and `range.formulas`, then call `workbook.recalculate()` before reading computed values.
- Export an `.xlsx` with `await SpreadsheetFile.exportXlsx(workbook)`.

## Workflow

- Model the workbook structure first: sheets, headers, and key formulas.
- Use formulas instead of copying computed values when the sheet should remain editable.
- Recalculate before exporting or reading formula results.
- When editing an existing workbook, load it first and preserve unaffected sheets.

## Reference Map

- [`references/workbook.md`](./references/workbook.md) for workbook lifecycle and worksheet basics.
- [`references/ranges.md`](./references/ranges.md) for A1 addressing, values, formulas, and formatting.
