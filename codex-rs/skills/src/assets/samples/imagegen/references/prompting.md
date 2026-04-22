# Prompting best practices

These prompting principles are shared by both top-level modes of the skill:
- built-in `image_gen` tool (default)
- explicit `scripts/image_gen.py` CLI fallback

This file is about prompt structure, specificity, and iteration. Fallback-only execution controls such as `quality`, `input_fidelity`, masks, output format, and output paths live in the fallback docs.

## Contents
- [Structure](#structure)
- [Specificity policy](#specificity-policy)
- [Allowed and disallowed augmentation](#allowed-and-disallowed-augmentation)
- [Composition and layout](#composition-and-layout)
- [Constraints and invariants](#constraints-and-invariants)
- [Text in images](#text-in-images)
- [Input images and references](#input-images-and-references)
- [Iterate deliberately](#iterate-deliberately)
- [Transparent images](#transparent-images)
- [Fallback-only execution controls](#fallback-only-execution-controls)
- [Use-case tips](#use-case-tips)
- [Where to find copy/paste recipes](#where-to-find-copypaste-recipes)

## Structure
- Use a consistent order: scene/backdrop -> subject -> key details -> constraints -> output intent.
- Include intended use (ad, UI mock, infographic) to set the level of polish.
- For complex requests, use short labeled lines instead of one long paragraph.

## Specificity policy
- If the user prompt is already specific and detailed, normalize it into a clean spec without adding creative requirements.
- If the prompt is generic, you may add tasteful detail when it materially improves the output.
- Treat examples in `sample-prompts.md` as fully-authored recipes, not as the default amount of augmentation to add to every request.
- For photorealism, include `photorealistic` directly when that is the goal, plus concrete real-world texture such as pores, wrinkles, fabric wear, material grain, or imperfect everyday detail.

## Allowed and disallowed augmentation

Allowed augmentation for generic prompts:
- composition and framing cues
- intended-use or polish-level hints
- practical layout guidance
- reasonable scene concreteness that supports the request

Do not add:
- extra characters, props, or objects that are not implied
- brand palettes, slogans, or story beats that are not implied
- arbitrary side-specific placement unless the surrounding layout supports it

## Composition and layout
- Specify framing and viewpoint (close-up, wide, top-down) and placement only when it materially helps.
- Call out negative space if the asset clearly needs room for UI or copy.
- Avoid making left/right layout decisions unless the user or surrounding layout supports them.
- For people, describe body framing, scale, gaze, and object interactions when they matter (`full body visible`, `looking down at the book`, `hands naturally gripping the handlebars`).

## Constraints and invariants
- State what must not change (`keep background unchanged`).
- For edits, say `change only X; keep Y unchanged` and repeat invariants on every iteration to reduce drift.

## Text in images
- Put literal text in quotes or ALL CAPS and specify typography (font style, size, color, placement).
- Spell uncommon words letter-by-letter if accuracy matters.
- For in-image copy, require verbatim rendering and no extra characters.
- In CLI fallback mode, use `medium` or `high` quality for small text, dense infographics, data-heavy slides, multi-font layouts, legends, axes, and footnotes.

## Input images and references
- Do not assume that every provided image is an edit target.
- Label each image by index and role (`Image 1: edit target`, `Image 2: style reference`).
- If the user provides images for style, composition, or mood guidance and does not ask to modify them, treat the request as generation with references.
- If the user asks to preserve an existing image while changing specific parts, treat the request as an edit.
- For compositing, describe how the images interact (`place the subject from Image 2 into Image 1`).

## Iterate deliberately
- Start with a clean base prompt, then make small single-change edits.
- Re-specify critical constraints when you iterate.
- Prefer one targeted follow-up at a time over rewriting the whole prompt.

## Transparent images
- Use built-in `image_gen` first for transparent-image requests. If the subject is clearly too complex for chroma-key removal, explain the fallback and ask before switching to CLI.
- Prompt for a perfectly flat solid chroma-key background, usually `#00ff00`; use `#ff00ff` when the subject is green, and avoid key colors that appear in the subject.
- Explicitly prohibit shadows, gradients, floor planes, reflections, texture, and lighting variation in the background.
- Ask for crisp edges, generous padding, and no use of the key color inside the subject.
- After generation, remove the background locally with `python "${CODEX_HOME:-$HOME/.codex}/skills/.system/imagegen/scripts/remove_chroma_key.py" --input <source> --out <final.png> --auto-key border --soft-matte --transparent-threshold 12 --opaque-threshold 220 --despill` and validate the alpha result before shipping it.
- Use soft matte and despill for antialiased edges; hard tolerance-only removal is mainly for flat pixel-art or exact-color fixtures.
- Use CLI `gpt-image-1.5 --background transparent --output-format png` only after the user explicitly confirms the fallback, or when the user already explicitly requested `gpt-image-1.5`, `scripts/image_gen.py`, or CLI fallback. Ask first for true/native transparency requests, failed chroma-key validation, or complex transparent subjects such as hair, fur, glass, smoke, liquids, translucent materials, reflective objects, or soft shadows.

## Fallback-only execution controls
- `quality`, `input_fidelity`, explicit masks, output format, and output paths are fallback-only execution controls.
- Do not assume they are built-in `image_gen` tool arguments.
- If the user explicitly chooses CLI fallback, see `references/cli.md` and `references/image-api.md` for those controls.
- In CLI fallback mode, `gpt-image-2` is the default. It supports `quality=low|medium|high|auto`; use `low` for fast drafts and thumbnails, and move to `medium`, `high`, or `auto` for final assets.
- `gpt-image-2` always uses high fidelity for image inputs, so do not set `input_fidelity` with that model.
- If a transparent request needs true CLI transparency, ask before using `gpt-image-1.5` unless the user already explicitly chose it. Explain that built-in chroma-key removal is the default path, but `gpt-image-2` does not support `background=transparent`.
- If the user asks for 4K-style output with `gpt-image-2`, use `3840x2160` for landscape or `2160x3840` for portrait.

## Use-case tips
Generate:
- photorealistic-natural: Prompt as if a real photo is captured in the moment; use photography language (lens, lighting, framing); call for real texture; avoid over-stylized polish unless requested.
- product-mockup: Describe the product/packaging and materials; ensure clean silhouette and label clarity; if in-image text is needed, require verbatim rendering and specify typography.
- ui-mockup: Describe the target fidelity first (shippable mockup or low-fi wireframe), then focus on layout, hierarchy, and practical UI elements; avoid concept-art language.
- infographic-diagram: Define the audience and layout flow; label parts explicitly; require verbatim text; prefer higher quality in CLI mode for dense labels.
- logo-brand: Keep it simple and scalable; ask for a strong silhouette and balanced negative space; avoid decorative flourishes unless requested.
- ads-marketing: Write like a creative brief; include brand positioning, audience, desired vibe, scene, and exact tagline if text must appear.
- productivity-visual: Name the exact artifact (slide, chart, workflow diagram), define the canvas and hierarchy, provide real labels/data, and ask for readable typography and polished spacing.
- scientific-educational: Define audience, lesson objective, required labels, scientific constraints, arrows, and scan-friendly whitespace.
- illustration-story: Define panels or scene beats; keep each action concrete.
- stylized-concept: Specify style cues, material finish, and rendering approach (3D, painterly, clay) without inventing new story elements.
- historical-scene: State the location/date and required period accuracy; constrain clothing, props, and environment to match the era.

Edit:
- text-localization: Change only the text; preserve layout, typography, spacing, and hierarchy; no extra words or reflow unless needed.
- identity-preserve: Lock identity (face, body, pose, hair, expression); change only the specified elements; match lighting and shadows.
- precise-object-edit: Specify exactly what to remove/replace; preserve surrounding texture and lighting; keep everything else unchanged.
- lighting-weather: Change only environmental conditions (light, shadows, atmosphere, precipitation); keep geometry, framing, and subject identity.
- background-extraction: For simple opaque subjects, request a clean cutout on a perfectly flat chroma-key background; crisp silhouette; generous padding; no shadows; no halos; preserve label text exactly; no restyling. Ask before using true CLI transparency for complex subjects.
- style-transfer: Specify style cues to preserve (palette, texture, brushwork) and what must change; add `no extra elements` to prevent drift.
- compositing: Reference inputs by index; specify what moves where; match lighting, perspective, and scale; keep the base framing unchanged.
- sketch-to-render: Preserve layout, proportions, and perspective; choose materials and lighting that support the supplied sketch without adding new elements.

## Where to find copy/paste recipes
For copy/paste prompt specs (examples only), see `references/sample-prompts.md`. This file focuses on principles, specificity, and iteration patterns.
