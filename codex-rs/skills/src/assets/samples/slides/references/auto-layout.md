# Auto Layout

Use auto-layout helpers when several shapes should align or distribute predictably.

```js
const title = slide.shapes.add({ geometry: "rect" });
const subtitle = slide.shapes.add({ geometry: "rect" });

title.position = { width: 720, height: 72 };
subtitle.position = { width: 720, height: 40 };

slide.autoLayout([title, subtitle], {
  direction: AutoLayoutDirection.vertical,
  frame: "slide",
  align: AutoLayoutAlign.topLeft,
  horizontalPadding: 72,
  verticalPadding: 64,
  verticalGap: 12,
});
```

Useful enums:

- `AutoLayoutDirection.vertical`
- `AutoLayoutDirection.horizontal`
- `AutoLayoutAlign.topLeft`
- `AutoLayoutAlign.center`
- `AutoLayoutAlign.bottomLeft`

Prefer auto-layout for title stacks, card grids, and footer or header placement instead of hand-adjusting every `left` and `top`.
