import { extractTextFromDocument } from "../src/index";

describe("extractTextFromDocument", () => {
  it("returns empty string for missing body", () => {
    expect(extractTextFromDocument({})).toBe("");
  });

  it("concatenates text runs from paragraphs", () => {
    const doc = {
      body: {
        content: [
          {
            paragraph: {
              elements: [
                { textRun: { content: "Hello, " } },
                { textRun: { content: "world!" } },
              ],
            },
          },
          {
            paragraph: {
              elements: [{ textRun: { content: "\nSecond line." } }],
            },
          },
        ],
      },
    };

    expect(extractTextFromDocument(doc)).toBe("Hello, world!\nSecond line.");
  });

  it("appends link URLs from text styles when missing in content", () => {
    const link =
      "https://docs.google.com/spreadsheets/d/1th8gnd4M6pvFRawdP3oTDlB72e_vVbOnnWZLMTsKU90/edit?gid=402341878#gid=402341878";
    const doc = {
      body: {
        content: [
          {
            paragraph: {
              elements: [
                {
                  textRun: {
                    content: "See the sheet",
                    textStyle: { link: { url: link } },
                  },
                },
                { textRun: { content: "\nNext line" } },
              ],
            },
          },
        ],
      },
    };

    expect(extractTextFromDocument(doc)).toBe(
      `See the sheet (${link})\nNext line`,
    );
  });

  it("renders rich link chips using title with URL appended", () => {
    const link =
      "https://docs.google.com/spreadsheets/d/1th8gnd4M6pvFRawdP3oTDlB72e_vVbOnnWZLMTsKU90/edit?gid=402341878#gid=402341878";
    const doc = {
      body: {
        content: [
          {
            paragraph: {
              elements: [
                {
                  richLink: {
                    richLinkProperties: {
                      title: "React/Next.js Critical Vulnerability Patching",
                      uri: link,
                    },
                  },
                },
              ],
            },
          },
        ],
      },
    };

    expect(extractTextFromDocument(doc)).toBe(
      `React/Next.js Critical Vulnerability Patching (${link})`,
    );
  });

  it("falls back to rich link URI when title is missing", () => {
    const link =
      "https://docs.google.com/spreadsheets/d/1th8gnd4M6pvFRawdP3oTDlB72e_vVbOnnWZLMTsKU90/edit?gid=402341878#gid=402341878";
    const doc = {
      body: {
        content: [
          {
            paragraph: {
              elements: [
                {
                  richLink: {
                    richLinkProperties: {
                      title: "",
                      uri: link,
                    },
                  },
                },
              ],
            },
          },
        ],
      },
    };

    expect(extractTextFromDocument(doc)).toBe(link);
  });

  it("does not duplicate link URLs already in content", () => {
    const link = "https://example.com/already-there";
    const doc = {
      body: {
        content: [
          {
            paragraph: {
              elements: [
                {
                  textRun: {
                    content: `Link present ${link}`,
                    textStyle: { link: { url: link } },
                  },
                },
              ],
            },
          },
        ],
      },
    };

    expect(extractTextFromDocument(doc)).toBe(`Link present ${link}`);
  });
});
