---
name: slides
description: Build, edit, render, import, and export presentation decks with the preloaded @oai/artifact-tool JavaScript surface through the artifacts tool.
metadata:
  short-description: Use the artifacts tool to create and edit slide decks in JavaScript
---

# Slides

Use this skill when the user wants to create or modify presentation decks with the `artifacts` tool.

## Tool Contract

- Use the `artifacts` tool.
- Send raw JavaScript only. Do not send JSON objects, quoted code, or markdown fences.
- This tool runs plain JavaScript in Node, not TypeScript. Do not use type annotations, `type`, `interface`, or `import type`.
- Do not write `import { ... } from "@oai/artifact-tool"`. The `@oai/artifact-tool` module surface is already preloaded on `globalThis`.
- Named exports such as `Presentation`, `PresentationFile`, `FileBlob`, `AutoLayoutAlign`, and `AutoLayoutDirection` are available directly.
- The full module is also available as `artifactTool`, `artifacts`, and `codexArtifacts`.
- You may still import Node built-ins such as `node:fs/promises` when you need to write preview bytes to disk.
- Save outputs under a user-visible path such as `artifacts/quarterly-update.pptx` or `artifacts/slide-1.png`.

## Quick Start

```js
const presentation = Presentation.create({
  slideSize: { width: 960, height: 540 },
});

const slide = presentation.slides.add();
slide.background.fill = "background1";

const title = slide.shapes.add({
  geometry: "roundRect",
  position: { left: 80, top: 72, width: 800, height: 96 },
  fill: "accent1",
});
title.text = "Q2 Product Update";

const subtitle = slide.shapes.add({
  geometry: "rect",
  position: { left: 80, top: 196, width: 800, height: 48 },
});
subtitle.text = "Launch status, reliability, and next milestones";

const pptxBlob = await PresentationFile.exportPptx(presentation);
await pptxBlob.save("artifacts/q2-product-update.pptx");
```

## Runtime Guardrails

- Prefer `slide.elements.charts.add("line", { position: ... })` for charts. The runtime chart surface is element-based; `slide.charts.add(...)` is not the reliable entry point for authoring new charts in this skill.
- After creating a chart element, set properties on the returned chart object:

```js
const chart = slide.elements.charts.add("line", {
  position: { left: 80, top: 180, width: 640, height: 320 },
});
chart.title = "Horsepower";
chart.categories = ["1964", "1973", "1989", "2024"];
const series = chart.series.add("911");
series.values = [130, 210, 247, 518];
chart.hasLegend = false;
```

- For local or fetched images that must survive preview rendering, embed bytes rather than passing only a file path or URL. The most reliable pattern is an `ArrayBuffer` plus `contentType`:

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

- If you fetch an image in-script, save or convert it to bytes first, then pass the `ArrayBuffer` into `slide.elements.images.add(...)`. Do not assume `path`, `url`, `src`, or a Node `Buffer` will preview correctly.
- PPTX export can still inherit layout placeholders such as `Title 1`, `Subtitle 2`, date/footer placeholders, or PowerPoint's `Click to add title` boxes. If the deck is meant to be fully custom, strip placeholder shapes before final export:

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

## Common Patterns

- Create a new deck with `Presentation.create({ slideSize })`.
- Import an existing deck with `await PresentationFile.importPptx(await FileBlob.load("deck.pptx"))`.
- Add slides with `presentation.slides.add()` or `presentation.slides.insert({ after, layout })`.
- Add content with `slide.shapes.add(...)`, `slide.tables.add(...)`, `slide.elements.charts.add(...)`, and `slide.elements.images.add(...)` when you need preview-safe embedded images.
- Render a preview with `await presentation.export({ slide, format: "png", scale: 2 })`, then write `new Uint8Array(await blob.arrayBuffer())` with `node:fs/promises`.
- Export a `.pptx` with `await PresentationFile.exportPptx(presentation)`.

## Workflow

- Start with the smallest script that creates or imports the deck.
- Do not begin by checking whether the local artifacts runtime package or cache exists. Assume the `artifacts` tool is ready and start authoring immediately; only investigate runtime installation or packaging if the tool fails before your slide code runs.
- If the API surface is unclear, do a tiny probe first: create one slide, add one shape, set `text` or `textStyle`, export one PNG, and inspect the result before scaling up to the full deck.
- Save the `.pptx` after meaningful milestones so the user can inspect output.
- Prefer short copy and a reusable component system over text-heavy layouts; the preview loop is much faster than rescuing a dense slide after export.
- Text boxes do not reliably auto-fit. If copy might wrap, give the shape extra height up front, then shorten the copy or enlarge the box until the rendered PNG shows clear padding on every edge.
- Deliberately check text contrast against the actual fill or image behind it. Do not leave dark text on dark fills, light text on light fills, or any pairing that is hard to read at presentation distance.
- Treat text containment as a hard requirement. No text should overlap other elements, sit underneath another object, spill outside its geometry, or run off the slide edge.
- If text sits inside a box, make the alignment look intentional. Center it only when the layout calls for centered copy; otherwise choose left or right alignment that matches the composition and leaves balanced padding.
- If you are using charts or external images, run a tiny end-to-end preview probe before building the full deck. One chart with one series and one embedded image is enough to validate the runtime surface.
- If the deck should not use PowerPoint placeholders, inspect imported slide shapes for inherited layout items before the final export. Delete shapes whose names match built-in placeholder labels or that expose `placeholderType`.
- If layout is repetitive, use `slide.autoLayout(...)` rather than hand-tuning every coordinate.
- QA with rendered PNG previews before handoff. In practice this is a more reliable quick check than importing the generated `.pptx` back into the runtime and inspecting round-tripped objects.
- Final QA means checking every rendered slide for contrast, intentional alignment, text superposition, clipped text, overflowing text, and inherited placeholder boxes. If text is hard to read against its background, if one text box overlaps another, if stacked text becomes hard to read, if any line touches a box edge, if text looks misaligned inside its box, or if PowerPoint shows `Click to add ...` placeholders, fix the layout or delete the inherited placeholder shapes and re-export before handoff.
- When editing an existing file, load it first, mutate only the requested slides or elements, then export a new `.pptx`.

## Reference Map

- [`references/presentation.md`](./references/presentation.md) for the core `Presentation` and `PresentationFile` lifecycle.
- [`references/auto-layout.md`](./references/auto-layout.md) for deterministic layout helpers and alignment enums.
