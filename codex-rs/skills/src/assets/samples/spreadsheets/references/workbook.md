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

## Output

Prefer saving generated files into `artifacts/` so the user can inspect the workbook directly.
