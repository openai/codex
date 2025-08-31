# Slide CLI (MVP)

Lightweight terminal agent to generate Markdown slides via chat. Ships as a Node launcher that executes platform-specific native binaries.

## Quickstart
```
npm i -g @yourorg/slide
slide
slide "営業向け提案 10枚 日本語で"
```

## Preview TUI (MVP)
```
slide preview slides/<file>.md
```

- Navigate: ←/→ (or j/k)
- Quit: q

## Notes
- Generates Markdown into `slides/` as `<timestamp>_<slug>.md`.
- PPTX/PDF conversion is out of scope for MVP (use external tools).
