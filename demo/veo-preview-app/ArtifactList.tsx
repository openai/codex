import React, { useMemo } from "react";
import { ArtifactType, GeneratedArtifact } from "./types";

interface Props {
  artifacts: GeneratedArtifact[];
}

const renderContent = (artifact: GeneratedArtifact) => {
  if (artifact.type !== ArtifactType.VEO_VIDEO) {
    return <p>{artifact.content}</p>;
  }

  const parsed = JSON.parse(artifact.content) as {
    objectUrl?: string;
    downloadUri?: string;
    fileName?: string;
  };

  return (
    <div>
      <video
        src={parsed.objectUrl}
        controls
        style={{ width: "100%", maxWidth: 720, display: "block" }}
        preload="metadata"
      />
      <div style={{ marginTop: 8 }}>
        <span style={{ marginRight: 8 }}>Video generated</span>
        {parsed.downloadUri ? (
          <a href={parsed.downloadUri} download={parsed.fileName ?? "veo-preview.mp4"}>
            Download from source
          </a>
        ) : null}
      </div>
    </div>
  );
};

export const ArtifactList: React.FC<Props> = ({ artifacts }) => {
  const sortedArtifacts = useMemo(
    () => [...artifacts].sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime()),
    [artifacts]
  );

  if (!sortedArtifacts.length) {
    return <p>No artifacts yet.</p>;
  }

  return (
    <div style={{ display: "grid", gap: 16 }}>
      {sortedArtifacts.map((artifact) => (
        <article key={artifact.id} style={{ padding: 12, border: "1px solid #ddd", borderRadius: 8 }}>
          <header style={{ marginBottom: 8 }}>
            <strong>{artifact.type}</strong>
            <span style={{ marginLeft: 8, color: "#666" }}>
              {artifact.createdAt.toLocaleString()}
            </span>
          </header>
          {renderContent(artifact)}
        </article>
      ))}
    </div>
  );
};
