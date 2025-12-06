import React, { useCallback, useMemo, useState } from "react";
import { ASPECT_RATIOS, DEFAULT_ASPECT_RATIO } from "./constants";
import { generateVeoVideo } from "./geminiService";
import { ArtifactType, GeneratedArtifact, VeoParams } from "./types";
import { ArtifactList } from "./ArtifactList";

const emptyPrompt = "Generate a dynamic preview using the uploaded image.";

export const App: React.FC = () => {
  const [prompt, setPrompt] = useState<string>(emptyPrompt);
  const [aspectRatio, setAspectRatio] = useState<string>(DEFAULT_ASPECT_RATIO);
  const [imageFile, setImageFile] = useState<File | null>(null);
  const [artifacts, setArtifacts] = useState<GeneratedArtifact[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleFileChange: React.ChangeEventHandler<HTMLInputElement> = useCallback(
    (event) => {
      const file = event.target.files?.[0];
      setImageFile(file ?? null);
      setError(null);
    },
    []
  );

  const handleGenerate = useCallback(async () => {
    if (!imageFile) {
      setError("Upload an image to guide Veo video generation.");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const veoParams: VeoParams = {
        prompt,
        aspectRatio,
        imageFile,
        resolution: "720p",
      };
      const videoArtifact = await generateVeoVideo(veoParams);
      setArtifacts((prev) => [videoArtifact, ...prev]);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Video generation failed.";
      console.error("[VEO] generation error", err);
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, [aspectRatio, imageFile, prompt]);

  const hasVideoArtifacts = useMemo(
    () => artifacts.some((artifact) => artifact.type === ArtifactType.VEO_VIDEO),
    [artifacts]
  );

  return (
    <main style={{ padding: "1rem", maxWidth: 920, margin: "0 auto" }}>
      <h1>Veo 3.1 Fast Preview</h1>
      <p>
        Upload an image and craft a prompt. The app will request a video preview using
        the {""}
        <code>veo-3.1-fast-generate-preview</code> model, polls until ready, and then streams
        the result via an object URL.
      </p>

      <section style={{ marginBottom: 16 }}>
        <label style={{ display: "block", marginBottom: 8 }}>
          Prompt
          <textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            rows={3}
            style={{ width: "100%", padding: 8, marginTop: 4 }}
            placeholder={emptyPrompt}
          />
        </label>

        <label style={{ display: "block", marginBottom: 8 }}>
          Aspect Ratio
          <select
            value={aspectRatio}
            onChange={(e) => setAspectRatio(e.target.value)}
            style={{ width: "100%", padding: 8, marginTop: 4 }}
          >
            {ASPECT_RATIOS.map((ratio) => (
              <option key={ratio.value} value={ratio.value}>
                {ratio.label}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: "block", marginBottom: 8 }}>
          Upload image
          <input type="file" accept="image/*" onChange={handleFileChange} style={{ marginTop: 4 }} />
        </label>

        <button onClick={handleGenerate} disabled={isLoading} style={{ padding: "10px 16px" }}>
          {isLoading ? "Generating preview..." : "Generate Veo Video"}
        </button>

        {error ? <p style={{ color: "red", marginTop: 8 }}>{error}</p> : null}
      </section>

      {hasVideoArtifacts ? <h2>Generated videos</h2> : null}
      <ArtifactList artifacts={artifacts} />
    </main>
  );
};
