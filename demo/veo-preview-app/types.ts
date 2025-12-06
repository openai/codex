export enum ArtifactType {
  TEXT = "TEXT",
  IMAGE = "IMAGE",
  VEO_VIDEO = "VEO_VIDEO",
}

export interface GeneratedArtifact {
  id: string;
  type: ArtifactType;
  /**
   * Content for the artifact. For Veo videos we store a JSON string containing
   * the generated object URL and optional download URI to avoid bloating state
   * with raw bytes.
   */
  content: string;
  createdAt: Date;
  metadata?: Record<string, unknown>;
}

export interface VeoParams {
  prompt: string;
  aspectRatio: string;
  imageFile: File;
  resolution?: "720p" | "1080p";
}
