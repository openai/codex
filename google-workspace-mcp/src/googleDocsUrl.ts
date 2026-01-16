export type GoogleDocsDocumentRef = {
  documentId: string;
  tabId?: string;
};

export function parseGoogleDocsDocumentRef(
  value: string,
): GoogleDocsDocumentRef {
  const raw = value.trim();
  if (raw.length === 0) {
    return { documentId: "" };
  }

  try {
    const url = new URL(raw);
    const docIdMatch = url.pathname.match(
      /^\/document\/(?:u\/\d+\/)?d\/([^/]+)/,
    );
    if (!docIdMatch) {
      return { documentId: raw };
    }
    return {
      documentId: docIdMatch[1],
      tabId: url.searchParams.get("tab") ?? undefined,
    };
  } catch {
    // Not a URL; fall through.
  }

  const docIdMatch = raw.match(/\/document\/(?:u\/\d+\/)?d\/([^/?#]+)/);
  if (docIdMatch) {
    const tabMatch = raw.match(/[?&]tab=([^&#]+)/);
    return {
      documentId: docIdMatch[1],
      tabId: tabMatch ? decodeURIComponent(tabMatch[1]) : undefined,
    };
  }

  return { documentId: raw };
}
