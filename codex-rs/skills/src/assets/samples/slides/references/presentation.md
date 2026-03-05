# Presentation API

Use `Presentation` as the main facade for authoring and editing decks.

## Lifecycle

```js
const presentation = Presentation.create({
  slideSize: { width: 960, height: 540 },
});
```

- `Presentation.create()` creates a new empty deck.
- `await PresentationFile.importPptx(await FileBlob.load("deck.pptx"))` imports an existing deck.
- `await PresentationFile.exportPptx(presentation)` exports the deck as a saveable blob.
- When using this skill operationally, start by authoring with these APIs rather than checking local runtime package directories first. Runtime or package-cache inspection is a fallback for cases where the `artifacts` tool itself fails before deck code executes.

## Slides

- `presentation.slides.add()` appends a slide.
- `presentation.slides.insert({ after, layout, layoutId })` inserts relative to another slide.
- `presentation.slides.getItem(index)` returns a slide by zero-based index.
- `presentation.slides.items` exposes the backing collection when you need to iterate. Do not assume the collection is a normal array or that `.get(...)` exists.
- `presentation.setActiveSlide(slide)` and `presentation.getActiveSlide()` manage the active pointer.

## Content

- `slide.shapes.add({ geometry, position, fill, line })`
- `slide.elements.images.add(...)`
- `slide.tables.add(...)`
- `slide.elements.charts.add("line", { position })`

For the current runtime, charts are most reliable when created as slide elements:

```js
const chart = slide.elements.charts.add("line", {
  position: { left: 80, top: 180, width: 640, height: 320 },
});
chart.title = "Representative factory horsepower";
chart.categories = ["1964", "1973", "1989", "2024"];
const series = chart.series.add("911");
series.values = [130, 210, 247, 518];
chart.hasLegend = false;
```

For preview-safe images, prefer embedded bytes over a bare path or URL:

```js
const fs = await import("node:fs/promises");
const source = await fs.readFile("artifacts/porsche.jpg");
const imageBuffer = source.buffer.slice(
  source.byteOffset,
  source.byteOffset + source.byteLength,
);

slide.elements.images.add({
  blob: imageBuffer,
  contentType: "image/jpeg",
  position: { left: 500, top: 0, width: 460, height: 540 },
  fit: "cover",
});
```

Passing only `path`, `url`, `src`, or a Node `Buffer` may appear to work for export setup but fail during PNG preview rendering. Use an `ArrayBuffer` when the image must render in previews reliably.

When exporting a fully custom deck, also watch for inherited layout placeholders in the PPTX output. These often show up as empty shapes named `Title 1`, `Subtitle 2`, `Date Placeholder 3`, `Footer Placeholder 4`, or `Slide Number Placeholder 5`, and PowerPoint may render them as `Click to add ...` boxes even if preview PNGs look fine.

Use a cleanup pass before final export when placeholders are not wanted:

```js
const placeholderNames = new Set([
  "Title 1",
  "Subtitle 2",
  "Date Placeholder 3",
  "Footer Placeholder 4",
  "Slide Number Placeholder 5",
]);

for (const slide of presentation.slides.items) {
  const toDelete = slide.shapes.items.filter((shape) => {
    const name = shape.name ?? "";
    return placeholderNames.has(name) || Boolean(shape.placeholderType);
  });
  for (const shape of toDelete) {
    shape.delete();
  }
}
```

Collection access is not fully array-like:

- Use `slide.shapes.getItem(index)` or `slide.shapes.items[index]` for reads.
- Prefer probing with `Object.getOwnPropertyNames(Object.getPrototypeOf(...))` if you are unsure what a collection or element exposes.

## Text

Basic text works through the `text` property:

```js
const title = slide.shapes.add({
  geometry: "rect",
  position: { left: 80, top: 72, width: 800, height: 96 },
});
title.text = "Q2 Product Update";
```

Simple styling works through `textStyle`:

```js
title.textStyle = {
  fontSize: 28,
  color: "#101820",
  bold: true,
  italic: false,
};
```

Text boxes do not auto-resize to fit content. If the text might wrap, increase the shape height before you add more copy, and leave visible padding on all sides rather than sizing the box flush to the current line count.

Treat text placement as a layout quality check, not a best-effort outcome:

- Make sure the text color has strong contrast against the fill, image, or slide background behind it.
- Keep text fully inside the intended geometry with visible padding on each side.
- Do not allow text boxes to overlap each other or hide beneath charts, images, or decorative shapes.
- When text is inside a box or card, choose alignment intentionally. Center only when the design calls for centered copy; otherwise use the alignment that best matches the surrounding layout.

When you are authoring a new deck, confirm typography and clipping by rendering a PNG preview rather than assuming the slide will look right after export.

All geometry uses CSS pixels at 96 DPI.

## Preview

```js
const fs = await import("node:fs/promises");
const preview = await presentation.export({
  slide,
  format: "png",
  scale: 2,
});
const previewBytes = new Uint8Array(await preview.arrayBuffer());
await fs.writeFile("artifacts/slide-1.png", previewBytes);
```

- `format` is usually `"png"` or `"jpeg"`.
- `scale` increases output resolution.
- `presentation.export(...)` returns an image blob. It is not the same as `FileBlob`, so write it via `arrayBuffer()` and Node file I/O.
- For QA, rendered PNGs are the fastest trustworthy check for spacing, contrast, clipping, alignment, and unintended text overlap.
- Inspect every slide image before handoff. Small cards, labels, and footer text are the common overflow traps, and stacked text boxes can silently overlap after export. If one text block sits on top of another, if text becomes visually superposed, if text color blends into the background, if text alignment inside a box looks accidental, or if any line touches a box edge, resize the container, move the elements apart, change the styling, or shorten the copy and re-render.

## Output

Prefer saving artifacts into an `artifacts/` directory in the current working tree so the user can inspect outputs easily.
