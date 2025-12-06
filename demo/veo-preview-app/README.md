# Veo 3.1 Fast Preview Demo

A lightweight UI for experimenting with Veo video previews. Users can upload an
image, supply a prompt, and request a video via the `veo-3.1-fast-generate-preview`
model. The generated video is downloaded as a blob and displayed using an object
URL so large payloads are never stored in React state.

## Key behaviors
- Requires a selected API key from `window.aistudio.hasSelectedApiKey()` and also
  falls back to `process.env.API_KEY`.
- Creates a fresh `GoogleGenerativeAI` client immediately before Veo calls to use
  the newest key selection.
- Polls the long-running operation every 5 seconds until a video URI is ready.
- Converts the resulting blob to an object URL and stores that URL (plus the
  download URI) in the artifact content field to minimize memory usage.

## Files
- `types.ts` – shared artifact types, including `VEO_VIDEO` and `VeoParams`.
- `constants.ts` – model, aspect ratios, and polling interval defaults.
- `geminiService.ts` – Veo generation, upload, polling, and video download logic.
- `App.tsx` – React UI for prompt, aspect ratio, file upload, and generation.
- `ArtifactList.tsx` – renders generated video artifacts using `<video>`.
