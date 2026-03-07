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
- End every artifact run with a concise user-facing log that lists every file the script created or updated. Keep the message short and formatted for direct display, for example `Saved files` followed by one path per line.

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
const xlsxPath = "artifacts/revenue-model.xlsx";
await xlsxBlob.save(xlsxPath);

console.log([
  "Saved files",
  `- ${xlsxPath}`,
].join("\n"));
```

## Common Patterns

- Create a workbook with `Workbook.create()`.
- Import an existing workbook with `await SpreadsheetFile.importXlsx(await FileBlob.load("book.xlsx"))`.
- Add sheets with `workbook.worksheets.add(name)`.
- Address cells and ranges with A1 notation via `sheet.getRange("A1:C10")`.
- Set `range.values` and `range.formulas`, then call `workbook.recalculate()` before reading computed values.
- For charts, prefer creating the chart first, then populating it directly on the returned object. In practice, the reliable pattern is `const chart = sheet.charts.add("line"); chart.setPosition("A10", "H24"); chart.title = "..."; chart.categories = [...]; const series = chart.series.add("Name"); series.values = [...];`. Some other chart-construction styles can produce workbook objects that look valid in memory but export to empty or hidden charts in the final `.xlsx`.
- For worksheet images, prefer `sheet.images.add({ blob, contentType, anchor: { from: ..., to: ... } })`. The `blob` payload shape is the reliable path.
- Export an `.xlsx` with `await SpreadsheetFile.exportXlsx(workbook)`.

## Workflow

- Model the workbook structure first: sheets, headers, and key formulas.
- Do not begin by checking whether the local artifacts runtime package or cache exists. Assume the `artifacts` tool is ready and start authoring immediately; only investigate runtime installation or packaging if the tool fails before your workbook code runs.
- If the first `artifacts` run fails before your workbook code executes, treat it as runtime setup failure.
- Ask for approval to install Node or the required artifact runtime, explaining that a JavaScript runtime is required to build spreadsheet artifacts.
- After approval, install it and retry once. Do not loop on the same failed script.
- Use formulas instead of copying computed values when the sheet should remain editable.
- Recalculate before exporting or reading formula results.
- End the script with a final `console.log(...)` summary that names every file the run touched, using a compact user-facing format with one path per line.
- If the workbook includes charts or images, verify layout after export, not just in memory. A sheet-level render pass such as `await workbook.render({ sheet: index, format: "png" })` is a good QA step before handoff.
- Check where drawings land on the actual sheet. Merged cells and very tall autofit rows can push visible content far below the fold, so QA should confirm not only that a chart exists, but that it appears in an obvious on-sheet location.
- When editing an existing workbook, load it first and preserve unaffected sheets.

## Reference Map

- [`references/workbook.md`](./references/workbook.md) for workbook lifecycle and worksheet basics.
- [`references/ranges.md`](./references/ranges.md) for A1 addressing, values, formulas, and formatting.
