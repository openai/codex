import { GoogleGenerativeAI } from "@google/generative-ai";
import { ASPECT_RATIOS, DEFAULT_VEO_MODEL, POLL_INTERVAL_MS } from "./constants";
import { ArtifactType, GeneratedArtifact, VeoParams } from "./types";

declare global {
  interface Window {
    aistudio?: {
      hasSelectedApiKey?: () => Promise<boolean>;
      getSelectedApiKey?: () => string | undefined;
    };
  }
}

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const validateAspectRatio = (aspectRatio: string) => {
  const allowed = new Set(ASPECT_RATIOS.map((ratio) => ratio.value));
  if (!allowed.has(aspectRatio)) {
    throw new Error(`Unsupported aspect ratio: ${aspectRatio}`);
  }
};

const buildApiKey = async (): Promise<string> => {
  const selectedKey = await window?.aistudio?.hasSelectedApiKey?.();
  if (!selectedKey) {
    throw new Error("Please select an API key in AI Studio before requesting Veo videos.");
  }

  const keyFromStudio = window?.aistudio?.getSelectedApiKey?.();
  const apiKey = keyFromStudio ?? process.env.API_KEY;
  if (!apiKey) {
    throw new Error("API key missing. Set process.env.API_KEY or choose a key in AI Studio.");
  }

  return apiKey;
};

const fetchVideoBlob = async (uri: string, apiKey: string): Promise<Blob> => {
  const response = await fetch(uri, {
    headers: { Authorization: `Bearer ${apiKey}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to download generated video: ${response.statusText}`);
  }

  return response.blob();
};

const extractVideoUri = (operation: any): string | undefined => {
  return (
    operation?.response?.videos?.[0]?.uri ||
    operation?.result?.videos?.[0]?.uri ||
    operation?.videos?.[0]?.uri
  );
};

const uploadImage = async (ai: GoogleGenerativeAI, file: File) => {
  const mimeType = file.type || "image/png";
  const uploadResponse = await ai.files.upload({
    file,
    mimeType,
    displayName: file.name || "uploaded-image",
  });

  if (!uploadResponse?.file?.uri) {
    throw new Error("Image upload failed: missing URI from upload response.");
  }

  return uploadResponse.file.uri as string;
};

export const generateVeoVideo = async (params: VeoParams): Promise<GeneratedArtifact> => {
  validateAspectRatio(params.aspectRatio);
  const apiKey = await buildApiKey();
  const ai = new GoogleGenerativeAI(apiKey);

  console.info("[VEO] Uploading image for prompt...");
  const imageUri = await uploadImage(ai, params.imageFile);

  console.info("[VEO] Requesting video generation", {
    model: DEFAULT_VEO_MODEL,
    aspectRatio: params.aspectRatio,
  });

  const model = ai.getGenerativeModel({ model: DEFAULT_VEO_MODEL });
  const generation = await model.generateVideo({
    prompt: params.prompt,
    aspectRatio: params.aspectRatio,
    images: [imageUri],
    resolution: params.resolution ?? "720p",
  } as any);

  let operation = await generation.operation();
  console.info("[VEO] Polling operation", { name: operation.name });

  while (!operation.done) {
    await wait(POLL_INTERVAL_MS);
    operation = await ai.operations.getOperation({ name: operation.name });
  }

  const videoUri = extractVideoUri(operation);
  if (!videoUri) {
    throw new Error("Video generation completed but no video URI was found.");
  }

  console.info("[VEO] Downloading video blob", { videoUri });
  const blob = await fetchVideoBlob(videoUri, apiKey);
  const objectUrl = URL.createObjectURL(blob);

  return {
    id: crypto.randomUUID(),
    type: ArtifactType.VEO_VIDEO,
    content: JSON.stringify({ objectUrl, downloadUri: videoUri, fileName: "veo-preview.mp4" }),
    createdAt: new Date(),
    metadata: {
      model: DEFAULT_VEO_MODEL,
      aspectRatio: params.aspectRatio,
      resolution: params.resolution ?? "720p",
    },
  };
};
