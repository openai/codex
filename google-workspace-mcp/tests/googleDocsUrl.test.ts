import { parseGoogleDocsDocumentRef } from "../src/googleDocsUrl";

describe("parseGoogleDocsDocumentRef", () => {
  it("parses documentId and tabId from a Docs URL", () => {
    const ref = parseGoogleDocsDocumentRef(
      "https://docs.google.com/document/d/1MGpW_GOha3i7X5w0RXRHtZTUu89mlTo03vblbg0MnP0/edit?tab=t.ni76tvyn8x3r#heading=h.l4ix60ejb8d4",
    );

    expect(ref).toEqual({
      documentId: "1MGpW_GOha3i7X5w0RXRHtZTUu89mlTo03vblbg0MnP0",
      tabId: "t.ni76tvyn8x3r",
    });
  });

  it("parses /u/<n>/ URLs", () => {
    const ref = parseGoogleDocsDocumentRef(
      "https://docs.google.com/document/u/1/d/abc123_DEF-456/edit?tab=t.foo",
    );

    expect(ref).toEqual({ documentId: "abc123_DEF-456", tabId: "t.foo" });
  });

  it("passes through a bare documentId", () => {
    const ref = parseGoogleDocsDocumentRef("abc123_DEF-456");
    expect(ref).toEqual({ documentId: "abc123_DEF-456" });
  });
});
