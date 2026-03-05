# Workbook API

Use `Workbook` to create, edit, recalculate, and export spreadsheet artifacts.

## Lifecycle

```js
const workbook = Workbook.create();
const sheet = workbook.worksheets.add("Sheet1");
```

- `Workbook.create()` starts a new workbook.
- `await SpreadsheetFile.importXlsx(await FileBlob.load("book.xlsx"))` imports an existing workbook.
- `workbook.recalculate()` evaluates formulas.
- `await SpreadsheetFile.exportXlsx(workbook)` exports a saveable `.xlsx` blob.

## Worksheets

- `workbook.worksheets.add(name)` adds or returns a worksheet.
- `workbook.worksheets.getItem(nameOrIndex)` fetches an existing sheet.
- `workbook.worksheets.getActiveWorksheet()` returns the active sheet when relevant.

## Charts And Images

For charts that must survive `.xlsx` export reliably, prefer mutating the returned chart object directly:

```js
const chart = sheet.charts.add("line");
chart.setPosition("A10", "H24");
chart.title = "Cash runway";
chart.categories = ["Month 0", "Month 1", "Month 2"];

const series = chart.series.add("Savings");
series.values = [6000, 6850, 7700];
```

This authoring path is more reliable than some one-shot chart construction styles, which can produce workbook objects that appear fine in memory but export to empty or misplaced charts.

For worksheet images, prefer a `blob` payload:

```js
const fs = await import("node:fs/promises");
const bytes = await fs.readFile("artifacts/chart-preview.png");
const blob = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

sheet.images.add({
  blob,
  contentType: "image/png",
  anchor: {
    from: { row: 10, col: 0 },
    to: { row: 24, col: 8 },
  },
});
```

## QA

If the workbook includes charts or images, verify the final sheet layout after export, not just before export:

```js
const png = await workbook.render({ sheet: 0, format: "png" });
```

Use rendered previews to confirm both of these points:

- The chart or image actually appears in the exported workbook.
- It lands where a user will see it without scrolling far past the main table.

Merged cells and tall autofit rows can push drawings far below the fold even when the drawing exists and is correctly attached to the sheet.

## Output

Prefer saving generated files into `artifacts/` so the user can inspect the workbook directly.
